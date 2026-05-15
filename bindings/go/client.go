package lnd

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"math/rand/v2"
	"net"
	"net/http"
	"net/url"
	"slices"
	"strconv"
	"strings"
	"sync"
	"time"
)

const (
	DefaultTTLSeconds          uint64 = 30
	DefaultSSEKeepaliveSeconds uint64 = 15
)

type DiscoveryFilter struct {
	NetworkID string
	Service   string
	Tags      []string
}

func NewDiscoveryFilter(networkID string) DiscoveryFilter {
	return DiscoveryFilter{NetworkID: networkID}
}

func (f DiscoveryFilter) WithService(service string) DiscoveryFilter {
	f.Service = service
	return f
}

func (f DiscoveryFilter) AddTag(tag string) DiscoveryFilter {
	f.Tags = append(f.Tags, tag)
	return f
}

type AnnounceSpec struct {
	NetworkID            string            `json:"network_id"`
	NodeID               string            `json:"node_id"`
	Service              string            `json:"service"`
	DisplayName          string            `json:"display_name"`
	Port                 uint16            `json:"port"`
	LanAddrs             []string          `json:"lan_addrs,omitempty"`
	AutoLanAddrs         bool              `json:"auto_lan_addrs"`
	Tags                 []string          `json:"tags,omitempty"`
	Metadata             map[string]string `json:"metadata,omitempty"`
	TTLSeconds           uint64            `json:"ttl_secs"`
	AddressSelection     *AddressSelection `json:"address_selection,omitempty"`
}

func NewAnnounceSpec(networkID, nodeID, service, displayName string, port uint16) AnnounceSpec {
	return AnnounceSpec{
		NetworkID:    networkID,
		NodeID:       nodeID,
		Service:      service,
		DisplayName:  displayName,
		Port:         port,
		AutoLanAddrs: true,
		TTLSeconds:   DefaultTTLSeconds,
	}
}

func (s AnnounceSpec) AddLanAddr(addr string) AnnounceSpec {
	s.LanAddrs = append(s.LanAddrs, addr)
	return s
}

func (s AnnounceSpec) AddTag(tag string) AnnounceSpec {
	s.Tags = append(s.Tags, tag)
	return s
}

func (s AnnounceSpec) InsertMetadata(key, value string) AnnounceSpec {
	if s.Metadata == nil {
		s.Metadata = map[string]string{}
	}
	s.Metadata[key] = value
	return s
}

func (s AnnounceSpec) WithAddressSelection(selection AddressSelection) AnnounceSpec {
	s.AddressSelection = &selection
	return s
}

type AddressSelection struct {
	IncludePrivateIPv4   bool     `json:"include_private_ipv4"`
	IncludeLoopback      bool     `json:"include_loopback"`
	IncludeLinkLocalIPv4 bool     `json:"include_link_local_ipv4"`
	IncludeIPv6          bool     `json:"include_ipv6"`
	InterfaceAllowlist   []string `json:"interface_allowlist,omitempty"`
	InterfaceDenylist    []string `json:"interface_denylist,omitempty"`
}

func DefaultAddressSelection() AddressSelection {
	return AddressSelection{
		IncludePrivateIPv4: true,
	}
}

func (s AddressSelection) WithLoopback(on bool) AddressSelection {
	s.IncludeLoopback = on
	return s
}

func (s AddressSelection) WithIPv6(on bool) AddressSelection {
	s.IncludeIPv6 = on
	return s
}

func (s AddressSelection) WithPrivateIPv4(on bool) AddressSelection {
	s.IncludePrivateIPv4 = on
	return s
}

func (s AddressSelection) WithLinkLocalIPv4(on bool) AddressSelection {
	s.IncludeLinkLocalIPv4 = on
	return s
}

func (s AddressSelection) EnableInterface(name string) AddressSelection {
	s.InterfaceAllowlist = append(s.InterfaceAllowlist, name)
	return s
}

func (s AddressSelection) DisableInterface(name string) AddressSelection {
	s.InterfaceDenylist = append(s.InterfaceDenylist, name)
	return s
}

type LeaseInfo struct {
	Revision        uint64 `json:"revision"`
	TTLSeconds      uint64 `json:"ttl_secs"`
	ExpiresAtUnixMS uint64 `json:"expires_at_unix_ms"`
	LastSeenUnixMS  uint64 `json:"last_seen_unix_ms"`
}

type DiscoveredNode struct {
	NetworkID   string            `json:"network_id"`
	NodeID      string            `json:"node_id"`
	Service     string            `json:"service"`
	DisplayName string            `json:"display_name"`
	Port        uint16            `json:"port"`
	LanAddrs    []string          `json:"lan_addrs"`
	Tags        []string          `json:"tags"`
	Metadata    map[string]string `json:"metadata"`
	Lease       LeaseInfo         `json:"lease"`
}

type DiscoveryEvent struct {
	Type  string            `json:"type"`
	Nodes []DiscoveredNode  `json:"nodes,omitempty"`
	Node  *DiscoveredNode   `json:"node,omitempty"`
}

type DiscoveryEventEnvelope struct {
	Cursor *uint64        `json:"cursor"`
	Event  DiscoveryEvent `json:"event"`
}

type discoverResponse struct {
	Nodes  []DiscoveredNode `json:"nodes"`
	Cursor uint64           `json:"cursor"`
}

type apiError struct {
	Error string `json:"error"`
}

type Client struct {
	baseURL  string
	token    string
	http     *http.Client
	backoff  backoffConfig
	address  AddressSelection
}

type backoffConfig struct {
	min time.Duration
	max time.Duration
}

type ClientOption func(*Client)

func WithTimeout(timeout time.Duration) ClientOption {
	return func(c *Client) {
		c.http.Timeout = timeout
	}
}

func WithReconnectBackoff(min, max time.Duration) ClientOption {
	return func(c *Client) {
		c.backoff = backoffConfig{min: min, max: max}
	}
}

func NewClient(baseURL, bearerToken string, opts ...ClientOption) *Client {
	client := &Client{
		baseURL: strings.TrimRight(baseURL, "/"),
		token:   bearerToken,
		http: &http.Client{
			Timeout: 10 * time.Second,
		},
		backoff: backoffConfig{
			min: 500 * time.Millisecond,
			max: 15 * time.Second,
		},
		address: DefaultAddressSelection(),
	}
	for _, opt := range opts {
		opt(client)
	}
	return client
}

func (c *Client) Discover(ctx context.Context, filter DiscoveryFilter) ([]DiscoveredNode, error) {
	response, err := c.doDiscover(ctx, filter)
	if err != nil {
		return nil, err
	}
	return response.Nodes, nil
}

func (c *Client) AnnounceOnce(ctx context.Context, spec AnnounceSpec) (DiscoveredNode, error) {
	var node DiscoveredNode
	payload, err := c.buildAnnouncement(spec)
	if err != nil {
		return node, err
	}
	requestBody, err := json.Marshal(payload)
	if err != nil {
		return node, err
	}
	request, err := http.NewRequestWithContext(
		ctx,
		http.MethodPut,
		fmt.Sprintf("%s/v1/nodes/%s", c.baseURL, url.PathEscape(spec.NodeID)),
		bytes.NewReader(requestBody),
	)
	if err != nil {
		return node, err
	}
	request.Header.Set("Content-Type", "application/json")
	c.applyAuth(request)
	response, err := c.http.Do(request)
	if err != nil {
		return node, err
	}
	defer response.Body.Close()
	if err := decodeJSONResponse(response, &node); err != nil {
		return node, err
	}
	return node, nil
}

type announcePayload struct {
	NetworkID   string            `json:"network_id"`
	NodeID      string            `json:"node_id"`
	Service     string            `json:"service"`
	DisplayName string            `json:"display_name"`
	Port        uint16            `json:"port"`
	LanAddrs    []string          `json:"lan_addrs"`
	Tags        []string          `json:"tags,omitempty"`
	Metadata    map[string]string `json:"metadata,omitempty"`
	TTLSeconds  uint64            `json:"ttl_secs"`
}

type AnnounceHandle struct {
	cancel context.CancelFunc
	done   chan error
	once   sync.Once
}

func (h *AnnounceHandle) Close() error {
	h.once.Do(func() {
		h.cancel()
	})
	return <-h.done
}

func (c *Client) Announce(ctx context.Context, spec AnnounceSpec) *AnnounceHandle {
	runCtx, cancel := context.WithCancel(ctx)
	done := make(chan error, 1)
	go func() {
		done <- c.announceLoop(runCtx, spec)
	}()
	return &AnnounceHandle{
		cancel: cancel,
		done:   done,
	}
}

func (c *Client) announceLoop(ctx context.Context, spec AnnounceSpec) error {
	interval := time.Duration(maxUint64(spec.TTLSeconds/3, 1)) * time.Second
	attempt := uint32(0)
	for {
		if ctx.Err() != nil {
			return nil
		}
		if attempt > 0 {
			if err := sleepContext(ctx, c.backoffDelay(attempt)); err != nil {
				return nil
			}
		}
		_, err := c.AnnounceOnce(ctx, spec)
		if err != nil {
			attempt++
			continue
		}
		attempt = 0
		if err := sleepContext(ctx, withJitter(interval)); err != nil {
			return nil
		}
	}
}

type WatchHandle struct {
	cancel context.CancelFunc
	done   chan error
	once   sync.Once
}

func (h *WatchHandle) Close() error {
	h.once.Do(func() {
		h.cancel()
	})
	return <-h.done
}

func (c *Client) Watch(ctx context.Context, filter DiscoveryFilter, callback func(DiscoveryEventEnvelope)) *WatchHandle {
	runCtx, cancel := context.WithCancel(ctx)
	done := make(chan error, 1)
	go func() {
		done <- c.watchLoop(runCtx, filter, callback)
	}()
	return &WatchHandle{
		cancel: cancel,
		done:   done,
	}
}

func (c *Client) watchLoop(ctx context.Context, filter DiscoveryFilter, callback func(DiscoveryEventEnvelope)) error {
	var cursor *uint64
	var attempt uint32
	for {
		if ctx.Err() != nil {
			return nil
		}
		err := c.watchOnce(ctx, filter, cursor, callback, func(next uint64) {
			cursor = &next
			attempt = 0
		})
		if ctx.Err() != nil {
			return nil
		}
		if err == nil {
			continue
		}
		attempt++
		if sleepErr := sleepContext(ctx, c.backoffDelay(attempt)); sleepErr != nil {
			return nil
		}
	}
}

func (c *Client) watchOnce(
	ctx context.Context,
	filter DiscoveryFilter,
	cursor *uint64,
	callback func(DiscoveryEventEnvelope),
	onCursor func(uint64),
) error {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, c.watchURL(filter, cursor), nil)
	if err != nil {
		return err
	}
	request.Header.Set("Accept", "text/event-stream")
	c.applyAuth(request)
	response, err := c.http.Do(request)
	if err != nil {
		return err
	}
	defer response.Body.Close()

	if response.StatusCode == http.StatusConflict {
		callback(DiscoveryEventEnvelope{
			Cursor: cursor,
			Event:  DiscoveryEvent{Type: "reset"},
		})
		snapshot, err := c.doDiscover(ctx, filter)
		if err != nil {
			return err
		}
		onCursor(snapshot.Cursor)
		callback(DiscoveryEventEnvelope{
			Cursor: &snapshot.Cursor,
			Event: DiscoveryEvent{
				Type:  "snapshot",
				Nodes: snapshot.Nodes,
			},
		})
		return nil
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return decodeAPIError(response)
	}

	reader := bufio.NewReader(response.Body)
	for {
		payload, err := readSSEData(reader)
		if err != nil {
			if err == io.EOF || err == context.Canceled {
				return err
			}
			return err
		}
		if payload == "" {
			continue
		}
		var envelope DiscoveryEventEnvelope
		if err := json.Unmarshal([]byte(payload), &envelope); err != nil {
			return err
		}
		if envelope.Cursor != nil {
			onCursor(*envelope.Cursor)
		}
		callback(envelope)
		if envelope.Event.Type == "reset" {
			snapshot, err := c.doDiscover(ctx, filter)
			if err != nil {
				return err
			}
			onCursor(snapshot.Cursor)
			callback(DiscoveryEventEnvelope{
				Cursor: &snapshot.Cursor,
				Event: DiscoveryEvent{
					Type:  "snapshot",
					Nodes: snapshot.Nodes,
				},
			})
		}
	}
}

func (c *Client) doDiscover(ctx context.Context, filter DiscoveryFilter) (discoverResponse, error) {
	var responseBody discoverResponse
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, c.listURL(filter), nil)
	if err != nil {
		return responseBody, err
	}
	c.applyAuth(request)
	response, err := c.http.Do(request)
	if err != nil {
		return responseBody, err
	}
	defer response.Body.Close()
	if err := decodeJSONResponse(response, &responseBody); err != nil {
		return responseBody, err
	}
	return responseBody, nil
}

func (c *Client) listURL(filter DiscoveryFilter) string {
	values := url.Values{}
	values.Set("network_id", filter.NetworkID)
	if filter.Service != "" {
		values.Set("service", filter.Service)
	}
	for _, tag := range filter.Tags {
		values.Add("tag", tag)
	}
	return fmt.Sprintf("%s/v1/nodes?%s", c.baseURL, values.Encode())
}

func (c *Client) watchURL(filter DiscoveryFilter, cursor *uint64) string {
	values := url.Values{}
	values.Set("network_id", filter.NetworkID)
	if filter.Service != "" {
		values.Set("service", filter.Service)
	}
	for _, tag := range filter.Tags {
		values.Add("tag", tag)
	}
	if cursor != nil {
		values.Set("cursor", strconv.FormatUint(*cursor, 10))
	}
	return fmt.Sprintf("%s/v1/watch?%s", c.baseURL, values.Encode())
}

func (c *Client) applyAuth(request *http.Request) {
	if c.token != "" {
		request.Header.Set("Authorization", "Bearer "+c.token)
	}
}

func (c *Client) backoffDelay(attempt uint32) time.Duration {
	base := c.backoff.min
	if base <= 0 {
		base = 500 * time.Millisecond
	}
	max := c.backoff.max
	if max < base {
		max = base
	}
	delay := base * time.Duration(1<<minUint32(attempt, 10))
	if delay > max {
		delay = max
	}
	jitter := time.Duration(rand.Int64N(int64(base) + 1))
	delay += jitter
	if delay > max {
		delay = max
	}
	return delay
}

func withJitter(duration time.Duration) time.Duration {
	if duration <= 0 {
		return time.Second
	}
	jitter := time.Duration(rand.Int64N(int64(duration/5) + 1))
	return duration + jitter
}

func sleepContext(ctx context.Context, duration time.Duration) error {
	timer := time.NewTimer(duration)
	defer timer.Stop()
	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-timer.C:
		return nil
	}
}

func decodeJSONResponse(response *http.Response, out any) error {
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return decodeAPIError(response)
	}
	return json.NewDecoder(response.Body).Decode(out)
}

func decodeAPIError(response *http.Response) error {
	body, _ := io.ReadAll(response.Body)
	var apiErr apiError
	if err := json.Unmarshal(body, &apiErr); err == nil && apiErr.Error != "" {
		return fmt.Errorf(apiErr.Error)
	}
	return fmt.Errorf("%s: %s", response.Status, string(body))
}

func readSSEData(reader *bufio.Reader) (string, error) {
	var dataLines []string
	for {
		line, err := reader.ReadString('\n')
		if err != nil {
			return "", err
		}
		line = strings.TrimRight(line, "\r\n")
		if line == "" {
			return strings.Join(dataLines, "\n"), nil
		}
		if strings.HasPrefix(line, ":") {
			continue
		}
		if strings.HasPrefix(line, "data:") {
			dataLines = append(dataLines, strings.TrimSpace(strings.TrimPrefix(line, "data:")))
		}
	}
}

func maxUint64(a, b uint64) uint64 {
	if a > b {
		return a
	}
	return b
}

func minUint32(a, b uint32) uint32 {
	if a < b {
		return a
	}
	return b
}

func (c *Client) SetServerURL(baseURL string) *Client {
	c.baseURL = strings.TrimRight(baseURL, "/")
	return c
}

func (c *Client) SetBearerToken(token string) *Client {
	c.token = token
	return c
}

func (c *Client) SetIncludeLoopback(on bool) *Client {
	c.address.IncludeLoopback = on
	return c
}

func (c *Client) SetIncludeIPv6(on bool) *Client {
	c.address.IncludeIPv6 = on
	return c
}

func (c *Client) SetIncludePrivateIPv4(on bool) *Client {
	c.address.IncludePrivateIPv4 = on
	return c
}

func (c *Client) SetIncludeLinkLocalIPv4(on bool) *Client {
	c.address.IncludeLinkLocalIPv4 = on
	return c
}

func (c *Client) EnableInterface(name string) *Client {
	c.address.InterfaceAllowlist = append(c.address.InterfaceAllowlist, name)
	return c
}

func (c *Client) DisableInterface(name string) *Client {
	c.address.InterfaceDenylist = append(c.address.InterfaceDenylist, name)
	return c
}

func (c *Client) ClearInterfaceFilters() *Client {
	c.address.InterfaceAllowlist = nil
	c.address.InterfaceDenylist = nil
	return c
}

func (c *Client) ResolveAnnounceAddrs(spec AnnounceSpec) ([]string, error) {
	selection := c.mergedAddressSelection(spec.AddressSelection)
	addrs := append([]string{}, spec.LanAddrs...)
	if spec.AutoLanAddrs {
		autoAddrs, err := ResolveLanAddrsWithSelection(spec.Port, selection)
		if err != nil {
			return nil, err
		}
		addrs = append(addrs, autoAddrs...)
	}
	return dedupeStrings(addrs), nil
}

func (c *Client) buildAnnouncement(spec AnnounceSpec) (announcePayload, error) {
	lanAddrs, err := c.ResolveAnnounceAddrs(spec)
	if err != nil {
		return announcePayload{}, err
	}
	return announcePayload{
		NetworkID:   spec.NetworkID,
		NodeID:      spec.NodeID,
		Service:     spec.Service,
		DisplayName: spec.DisplayName,
		Port:        spec.Port,
		LanAddrs:    lanAddrs,
		Tags:        dedupeStrings(spec.Tags),
		Metadata:    spec.Metadata,
		TTLSeconds:  spec.TTLSeconds,
	}, nil
}

func (c *Client) mergedAddressSelection(override *AddressSelection) AddressSelection {
	if override == nil {
		return c.address
	}
	merged := c.address
	merged.IncludePrivateIPv4 = override.IncludePrivateIPv4
	merged.IncludeLoopback = override.IncludeLoopback
	merged.IncludeLinkLocalIPv4 = override.IncludeLinkLocalIPv4
	merged.IncludeIPv6 = override.IncludeIPv6
	if len(override.InterfaceAllowlist) > 0 {
		merged.InterfaceAllowlist = append([]string{}, override.InterfaceAllowlist...)
	}
	if len(override.InterfaceDenylist) > 0 {
		merged.InterfaceDenylist = append([]string{}, override.InterfaceDenylist...)
	}
	return merged
}

func (s AddressSelection) allowsInterface(name string) bool {
	allowed := len(s.InterfaceAllowlist) == 0 || slices.Contains(s.InterfaceAllowlist, name)
	denied := slices.Contains(s.InterfaceDenylist, name)
	return allowed && !denied
}

func (s AddressSelection) allowsIP(ip net.IP, isLoopback bool) bool {
	if ipv4 := ip.To4(); ipv4 != nil {
		if isLoopback {
			return s.IncludeLoopback
		}
		return ipv4.IsPrivate() && s.IncludePrivateIPv4 || ipv4.IsLinkLocalUnicast() && s.IncludeLinkLocalIPv4
	}
	if ip.IsLoopback() {
		return s.IncludeLoopback
	}
	return s.IncludeIPv6 && !ip.IsUnspecified()
}

func ResolveLanAddrsWithSelection(port uint16, selection AddressSelection) ([]string, error) {
	interfaces, err := net.Interfaces()
	if err != nil {
		return nil, err
	}
	addrs := make([]string, 0)
	seen := make(map[string]struct{})
	for _, iface := range interfaces {
		if !selection.allowsInterface(iface.Name) {
			continue
		}
		values, err := iface.Addrs()
		if err != nil {
			continue
		}
		isLoopback := (iface.Flags & net.FlagLoopback) != 0
		for _, addr := range values {
			var ip net.IP
			switch value := addr.(type) {
			case *net.IPNet:
				ip = value.IP
			case *net.IPAddr:
				ip = value.IP
			default:
				continue
			}
			if !selection.allowsIP(ip, isLoopback) {
				continue
			}
			normalized := ip
			if ipv4 := ip.To4(); ipv4 != nil {
				normalized = ipv4
			}
			key := net.JoinHostPort(normalized.String(), strconv.Itoa(int(port)))
			if _, ok := seen[key]; !ok {
				seen[key] = struct{}{}
				addrs = append(addrs, key)
			}
		}
	}
	return addrs, nil
}

func ResolvePrivateIPv4Addrs(port uint16) ([]string, error) {
	return ResolveLanAddrsWithSelection(port, DefaultAddressSelection())
}

func dedupeStrings(values []string) []string {
	if len(values) == 0 {
		return nil
	}
	sorted := append([]string{}, values...)
	slices.Sort(sorted)
	return slices.Compact(sorted)
}
