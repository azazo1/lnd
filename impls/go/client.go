package lnd

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"encoding/hex"
	"fmt"
	"hash/fnv"
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
	// DefaultTTLSeconds is the default lease duration used by NewAnnounceSpec.
	//
	// The background announce loop renews at about one third of this value.
	// Increase it to reduce server traffic, or lower it to remove stale peers sooner.
	DefaultTTLSeconds          uint64 = 30
	// DefaultSSEKeepaliveSeconds is the keepalive cadence emitted by the server watch stream.
	//
	// Watch clients do not need to send heartbeats themselves, but long running
	// reverse proxies should allow idle periods at least this long.
	DefaultSSEKeepaliveSeconds uint64 = 15
)

// DiscoveryFilter describes which peers should be listed or watched.
//
// NetworkID is optional and acts as a logical discovery domain. Service and
// Tags narrow the result set further. ReachabilityScopes require at least one
// overlap with the remote node.
//
// Example:
//
//	filter := lnd.NewDiscoveryFilter().
//		WithNetworkID("office-net").
//		WithService("_http._tcp").
//		AddTag("printer")
type DiscoveryFilter struct {
	NetworkID          *string  `json:"network_id,omitempty"`
	Service            string   `json:"service,omitempty"`
	Tags               []string `json:"tags,omitempty"`
	ReachabilityScopes []string `json:"reachability_scopes,omitempty"`
}

// NewDiscoveryFilter creates a minimal discovery filter.
func NewDiscoveryFilter() DiscoveryFilter {
	return DiscoveryFilter{}
}

// WithNetworkID sets the logical discovery domain and returns the updated copy.
func (f DiscoveryFilter) WithNetworkID(networkID string) DiscoveryFilter {
	f.NetworkID = &networkID
	return f
}

// WithService sets the required service name and returns the updated copy.
//
// Service names typically follow mDNS / DNS-SD service type conventions,
// for example "_http._tcp".
func (f DiscoveryFilter) WithService(service string) DiscoveryFilter {
	f.Service = service
	return f
}

// AddTag appends one required tag filter and returns the updated copy.
//
// A peer must contain every tag added to the filter to match.
func (f DiscoveryFilter) AddTag(tag string) DiscoveryFilter {
	f.Tags = append(f.Tags, tag)
	return f
}

// AddReachabilityScope appends one scope overlap filter and returns the updated copy.
func (f DiscoveryFilter) AddReachabilityScope(scope string) DiscoveryFilter {
	f.ReachabilityScopes = append(f.ReachabilityScopes, scope)
	return f
}

// AnnounceSpec describes one node registration payload.
//
// The spec can contain explicit LAN addresses, or it can ask the client to
// resolve addresses automatically from local interfaces.
//
// Example:
//
//	spec := lnd.NewAnnounceSpec("node-1", "_http._tcp", "Demo Node", 8080).
//		WithNetworkID("office-net").
//		AddTag("blue").
//		InsertMetadata("role", "api")
type AnnounceSpec struct {
	NetworkID            *string           `json:"network_id,omitempty"`
	NodeID               string            `json:"node_id"`
	Service              string            `json:"service"`
	DisplayName          string            `json:"display_name"`
	Port                 uint16            `json:"port"`
	LanAddrs             []string          `json:"lan_addrs,omitempty"`
	AutoLanAddrs         bool              `json:"auto_lan_addrs"`
	ReachabilityScopes   []string          `json:"reachability_scopes,omitempty"`
	AutoReachabilityScopes bool            `json:"auto_reachability_scopes"`
	Tags                 []string          `json:"tags,omitempty"`
	Metadata             map[string]string `json:"metadata,omitempty"`
	TTLSeconds           uint64            `json:"ttl_secs"`
	AddressSelection     *AddressSelection `json:"address_selection,omitempty"`
}

// NewAnnounceSpec creates an announce specification with sensible defaults.
//
// nodeID must remain stable across restarts. service identifies the protocol
// family and usually follows mDNS / DNS-SD service type conventions such as
// "_http._tcp". displayName is a human readable label, and port is the LAN
// service port advertised to peers.
//
// The returned spec enables automatic LAN address discovery and uses
// DefaultTTLSeconds unless overridden.
func NewAnnounceSpec(nodeID, service, displayName string, port uint16) AnnounceSpec {
	return AnnounceSpec{
		NodeID:                nodeID,
		Service:               service,
		DisplayName:           displayName,
		Port:                  port,
		AutoLanAddrs:          true,
		AutoReachabilityScopes: true,
		TTLSeconds:            DefaultTTLSeconds,
	}
}

// WithNetworkID sets the logical discovery domain and returns the updated copy.
func (s AnnounceSpec) WithNetworkID(networkID string) AnnounceSpec {
	s.NetworkID = &networkID
	return s
}

// AddLanAddr appends one explicit host:port address and returns the updated copy.
//
// Keep AutoLanAddrs enabled if you want explicit addresses to be merged with
// automatically discovered interfaces. Disable AutoLanAddrs to advertise only
// the addresses provided here.
func (s AnnounceSpec) AddLanAddr(addr string) AnnounceSpec {
	s.LanAddrs = append(s.LanAddrs, addr)
	return s
}

// AddReachabilityScope appends one explicit reachability scope.
func (s AnnounceSpec) AddReachabilityScope(scope string) AnnounceSpec {
	s.ReachabilityScopes = append(s.ReachabilityScopes, scope)
	return s
}

// AddTag appends one announce tag and returns the updated copy.
func (s AnnounceSpec) AddTag(tag string) AnnounceSpec {
	s.Tags = append(s.Tags, tag)
	return s
}

// InsertMetadata inserts one metadata key/value pair and returns the updated copy.
//
// Later calls with the same key replace the previous value.
func (s AnnounceSpec) InsertMetadata(key, value string) AnnounceSpec {
	if s.Metadata == nil {
		s.Metadata = map[string]string{}
	}
	s.Metadata[key] = value
	return s
}

// WithAddressSelection overrides automatic address selection for this spec.
//
// This per spec override takes precedence over the client default policy when
// automatic LAN address discovery is enabled.
func (s AnnounceSpec) WithAddressSelection(selection AddressSelection) AnnounceSpec {
	s.AddressSelection = &selection
	return s
}

// AddressSelection controls which local interfaces and IP families may be
// included in automatic LAN address discovery.
//
// The default policy only includes private IPv4 addresses. Loopback, IPv6 and
// link local IPv4 must be enabled explicitly.
type AddressSelection struct {
	IncludePrivateIPv4   bool     `json:"include_private_ipv4"`
	IncludeLoopback      bool     `json:"include_loopback"`
	IncludeLinkLocalIPv4 bool     `json:"include_link_local_ipv4"`
	IncludeIPv6          bool     `json:"include_ipv6"`
	InterfaceAllowlist   []string `json:"interface_allowlist,omitempty"`
	InterfaceDenylist    []string `json:"interface_denylist,omitempty"`
}

// DefaultAddressSelection returns the default automatic address selection policy.
//
// By default only private IPv4 addresses are included. The returned value can
// be refined with the WithXxx and Interface methods.
func DefaultAddressSelection() AddressSelection {
	return AddressSelection{
		IncludePrivateIPv4: true,
	}
}

// WithLoopback enables or disables loopback addresses in automatic selection.
func (s AddressSelection) WithLoopback(on bool) AddressSelection {
	s.IncludeLoopback = on
	return s
}

// WithIPv6 enables or disables IPv6 addresses in automatic selection.
func (s AddressSelection) WithIPv6(on bool) AddressSelection {
	s.IncludeIPv6 = on
	return s
}

// WithPrivateIPv4 enables or disables private IPv4 addresses in automatic selection.
func (s AddressSelection) WithPrivateIPv4(on bool) AddressSelection {
	s.IncludePrivateIPv4 = on
	return s
}

// WithLinkLocalIPv4 enables or disables link-local IPv4 addresses in automatic selection.
func (s AddressSelection) WithLinkLocalIPv4(on bool) AddressSelection {
	s.IncludeLinkLocalIPv4 = on
	return s
}

// EnableInterface appends one interface allowlist item.
func (s AddressSelection) EnableInterface(name string) AddressSelection {
	s.InterfaceAllowlist = append(s.InterfaceAllowlist, name)
	return s
}

// DisableInterface appends one interface denylist item.
func (s AddressSelection) DisableInterface(name string) AddressSelection {
	s.InterfaceDenylist = append(s.InterfaceDenylist, name)
	return s
}

// LeaseInfo contains server side lease state attached to a discovered node.
//
// Revision increases whenever the server updates this node record.
type LeaseInfo struct {
	Revision        uint64 `json:"revision"`
	TTLSeconds      uint64 `json:"ttl_secs"`
	ExpiresAtUnixMS uint64 `json:"expires_at_unix_ms"`
	LastSeenUnixMS  uint64 `json:"last_seen_unix_ms"`
}

// DiscoveredNode is the canonical peer record returned by list and watch calls.
type DiscoveredNode struct {
	NetworkID          *string           `json:"network_id"`
	NodeID             string            `json:"node_id"`
	Service            string            `json:"service"`
	DisplayName        string            `json:"display_name"`
	Port               uint16            `json:"port"`
	LanAddrs           []string          `json:"lan_addrs"`
	ReachabilityScopes []string          `json:"reachability_scopes"`
	Tags               []string          `json:"tags"`
	Metadata           map[string]string `json:"metadata"`
	Lease              LeaseInfo         `json:"lease"`
}

// DiscoveryEvent describes one watch stream event.
//
// Type is one of snapshot, upsert, remove, reset or keepalive.
type DiscoveryEvent struct {
	Type  string            `json:"type"`
	Nodes []DiscoveredNode  `json:"nodes,omitempty"`
	Node  *DiscoveredNode   `json:"node,omitempty"`
}

// DiscoveryEventEnvelope wraps a watch event with its latest resume cursor.
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

type backoffConfig struct {
	min time.Duration
	max time.Duration
}

// ClientOption customizes a Client created by NewClient.
type ClientOption func(*Client)

// WithTimeout sets the HTTP request timeout.
//
// Use this when list or announce requests may traverse slower proxies or links.
func WithTimeout(timeout time.Duration) ClientOption {
	return func(c *Client) {
		c.http.Timeout = timeout
	}
}

// WithReconnectBackoff sets the reconnect backoff range.
//
// min and max are used by the background watch and announce loops after
// transient failures.
func WithReconnectBackoff(min, max time.Duration) ClientOption {
	return func(c *Client) {
		c.backoff = backoffConfig{min: min, max: max}
	}
}

// Client is the high level Go SDK entry point for discovery, announce and watch.
//
// The client is safe to reuse across multiple operations. Default automatic
// address selection can be tuned with the SetIncludeXxx and Interface methods.
//
// Example:
//
//	client := lnd.NewClient("https://registry.example.com", "secret-token")
//	nodes, err := client.Discover(context.Background(), lnd.NewDiscoveryFilter("office-net"))
//	if err != nil {
//		return err
//	}
//	_ = nodes
type Client struct {
	baseURL  string
	token    string
	http     *http.Client
	backoff  backoffConfig
	address  AddressSelection
}

// DerivedNetworkID describes one locally derived discovery domain candidate.
//
// NetworkID is the stable identifier that can be sent to the server. Scope is
// a human readable subnet prefix such as 192.168.1.0/24.
type DerivedNetworkID struct {
	NetworkID string
	Scope     string
}

// NewClient creates a reusable Go SDK client.
//
// baseURL must point at an lnd server root, for example
// https://registry.example.com. bearerToken is optional and may be empty.
//
// The client does not contact the server during construction. Network and
// validation errors are returned by later API calls.
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

// Discover performs one HTTP list request and returns the matching peers.
//
// The method returns a slice of discovered nodes or an error when the request
// fails, the server rejects the filter, or the JSON response is invalid.
func (c *Client) Discover(ctx context.Context, filter DiscoveryFilter) ([]DiscoveredNode, error) {
	response, err := c.doDiscover(ctx, filter)
	if err != nil {
		return nil, err
	}
	return response.Nodes, nil
}

// ResolveNetworkID derives one local discovery domain identifier from the
// client's current automatic address selection policy.
//
// When multiple equally valid local subnets are visible, the method returns an
// error and the caller should pick an explicit network ID instead.
func (c *Client) ResolveNetworkID() (string, error) {
	return ResolveNetworkIDWithSelection(c.address)
}

// ListNetworkIDCandidates returns all locally derived discovery domain candidates.
func (c *Client) ListNetworkIDCandidates() ([]DerivedNetworkID, error) {
	return ListNetworkIDCandidates(c.address)
}

// ListReachabilityScopes returns all locally derived subnet scopes.
func (c *Client) ListReachabilityScopes() ([]string, error) {
	return ListReachabilityScopes(c.address)
}

// AnnounceOnce resolves addresses and performs one registration request.
//
// The returned node is the server normalized record after deduplication and
// lease metadata attachment. Errors include local address resolution failures,
// HTTP transport failures, authentication failures and invalid server JSON.
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
	NetworkID          *string           `json:"network_id,omitempty"`
	NodeID             string            `json:"node_id"`
	Service            string            `json:"service"`
	DisplayName        string            `json:"display_name"`
	Port               uint16            `json:"port"`
	LanAddrs           []string          `json:"lan_addrs"`
	ReachabilityScopes []string          `json:"reachability_scopes,omitempty"`
	Tags               []string          `json:"tags,omitempty"`
	Metadata           map[string]string `json:"metadata,omitempty"`
	TTLSeconds         uint64            `json:"ttl_secs"`
}

// AnnounceHandle manages a background announce loop started by Client.Announce.
//
// Call Close to stop renewals and wait for the goroutine to exit.
type AnnounceHandle struct {
	cancel context.CancelFunc
	done   chan error
	once   sync.Once
}

// Close stops the background announce loop and waits for it to exit.
//
// It returns the final loop error, or nil when the loop stopped cleanly.
func (h *AnnounceHandle) Close() error {
	h.once.Do(func() {
		h.cancel()
	})
	return <-h.done
}

// Announce starts a background announce loop.
//
// The loop keeps renewing the lease roughly every TTLSeconds/3 with jitter,
// and it reconnects with exponential backoff after transient failures.
//
// Call Close on the returned handle to stop it. The start itself is async, so
// initial errors are surfaced later by AnnounceHandle.Close.
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

// WatchHandle manages a background watch loop started by Client.Watch.
//
// Call Close to stop reconnection attempts and wait for the goroutine to exit.
type WatchHandle struct {
	cancel context.CancelFunc
	done   chan error
	once   sync.Once
}

// Close stops the background watch loop and waits for it to exit.
//
// It returns the final loop error, or nil when the watch stopped cleanly.
func (h *WatchHandle) Close() error {
	h.once.Do(func() {
		h.cancel()
	})
	return <-h.done
}

// Watch starts a reconnecting watch loop.
//
// callback receives parsed SSE events, including reset events and follow up
// snapshot resyncs. The loop automatically resumes from the latest cursor when
// the server supports replay.
//
// Call Close on the returned handle to stop the watch. As with Announce, later
// stream setup errors are reported by WatchHandle.Close.
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
	if filter.NetworkID != nil && *filter.NetworkID != "" {
		values.Set("network_id", *filter.NetworkID)
	}
	if filter.Service != "" {
		values.Set("service", filter.Service)
	}
	for _, tag := range filter.Tags {
		values.Add("tag", tag)
	}
	for _, scope := range filter.ReachabilityScopes {
		values.Add("scope", scope)
	}
	return fmt.Sprintf("%s/v1/nodes?%s", c.baseURL, values.Encode())
}

func (c *Client) watchURL(filter DiscoveryFilter, cursor *uint64) string {
	values := url.Values{}
	if filter.NetworkID != nil && *filter.NetworkID != "" {
		values.Set("network_id", *filter.NetworkID)
	}
	if filter.Service != "" {
		values.Set("service", filter.Service)
	}
	for _, tag := range filter.Tags {
		values.Add("tag", tag)
	}
	for _, scope := range filter.ReachabilityScopes {
		values.Add("scope", scope)
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

// SetServerURL updates the server base URL for subsequent requests.
//
// The value should be the server root URL without a trailing API path.
func (c *Client) SetServerURL(baseURL string) *Client {
	c.baseURL = strings.TrimRight(baseURL, "/")
	return c
}

// SetBearerToken updates the Bearer token for subsequent requests.
//
// Pass an empty string to disable Authorization headers.
func (c *Client) SetBearerToken(token string) *Client {
	c.token = token
	return c
}

// SetIncludeLoopback updates the default automatic address selection policy.
//
// This affects later address resolution unless a spec level override is set.
func (c *Client) SetIncludeLoopback(on bool) *Client {
	c.address.IncludeLoopback = on
	return c
}

// SetIncludeIPv6 updates the default automatic address selection policy.
func (c *Client) SetIncludeIPv6(on bool) *Client {
	c.address.IncludeIPv6 = on
	return c
}

// SetIncludePrivateIPv4 updates the default automatic address selection policy.
func (c *Client) SetIncludePrivateIPv4(on bool) *Client {
	c.address.IncludePrivateIPv4 = on
	return c
}

// SetIncludeLinkLocalIPv4 updates the default automatic address selection policy.
func (c *Client) SetIncludeLinkLocalIPv4(on bool) *Client {
	c.address.IncludeLinkLocalIPv4 = on
	return c
}

// EnableInterface appends one interface allowlist item to the client default policy.
//
// When the allowlist is non empty, only listed interfaces are considered.
func (c *Client) EnableInterface(name string) *Client {
	c.address.InterfaceAllowlist = append(c.address.InterfaceAllowlist, name)
	return c
}

// DisableInterface appends one interface denylist item to the client default policy.
//
// Deny rules override allow rules when an interface appears in both lists.
func (c *Client) DisableInterface(name string) *Client {
	c.address.InterfaceDenylist = append(c.address.InterfaceDenylist, name)
	return c
}

// ClearInterfaceFilters clears the client default interface allowlist and denylist.
func (c *Client) ClearInterfaceFilters() *Client {
	c.address.InterfaceAllowlist = nil
	c.address.InterfaceDenylist = nil
	return c
}

// ResolveAnnounceAddrs resolves the final address list for one announce specification.
//
// The result merges explicit LanAddrs with automatically discovered addresses
// when AutoLanAddrs is enabled, and removes duplicates before returning.
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

// ResolveReachabilityScopes resolves the final reachability scope list.
func (c *Client) ResolveReachabilityScopes(spec AnnounceSpec) ([]string, error) {
	scopes := append([]string{}, spec.ReachabilityScopes...)
	if spec.AutoReachabilityScopes {
		autoScopes, err := ListReachabilityScopes(c.mergedAddressSelection(spec.AddressSelection))
		if err != nil {
			return nil, err
		}
		scopes = append(scopes, autoScopes...)
	}
	return dedupeStrings(scopes), nil
}

func (c *Client) buildAnnouncement(spec AnnounceSpec) (announcePayload, error) {
	lanAddrs, err := c.ResolveAnnounceAddrs(spec)
	if err != nil {
		return announcePayload{}, err
	}
	reachabilityScopes, err := c.ResolveReachabilityScopes(spec)
	if err != nil {
		return announcePayload{}, err
	}
	return announcePayload{
		NetworkID:          spec.NetworkID,
		NodeID:             spec.NodeID,
		Service:            spec.Service,
		DisplayName:        spec.DisplayName,
		Port:               spec.Port,
		LanAddrs:           lanAddrs,
		ReachabilityScopes: reachabilityScopes,
		Tags:               dedupeStrings(spec.Tags),
		Metadata:           spec.Metadata,
		TTLSeconds:         spec.TTLSeconds,
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

// ResolveLanAddrsWithSelection resolves local addresses using the given selection policy.
//
// port is attached to every returned host address. The function skips interfaces
// whose addresses cannot be enumerated and deduplicates the final host:port list.
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

// ResolvePrivateIPv4Addrs resolves local private IPv4 addresses with the default policy.
//
// This helper is equivalent to ResolveLanAddrsWithSelection(port, DefaultAddressSelection()).
func ResolvePrivateIPv4Addrs(port uint16) ([]string, error) {
	return ResolveLanAddrsWithSelection(port, DefaultAddressSelection())
}

// ListNetworkIDCandidates derives candidate discovery domains from local interfaces.
//
// IPv4 candidates are built from ip/netmask subnet prefixes. IPv6 candidates
// are built from ip/prefix subnet prefixes when IPv6 is enabled by selection.
func ListNetworkIDCandidates(selection AddressSelection) ([]DerivedNetworkID, error) {
	interfaces, err := net.Interfaces()
	if err != nil {
		return nil, err
	}
	candidates := make([]DerivedNetworkID, 0)
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
			switch value := addr.(type) {
			case *net.IPNet:
				ip := value.IP
				if !selection.allowsIP(ip, isLoopback) {
					continue
				}
				scope, ok := deriveScopeFromIPNet(value)
				if !ok {
					continue
				}
				key := scope
				if _, exists := seen[key]; exists {
					continue
				}
				seen[key] = struct{}{}
				candidates = append(candidates, DerivedNetworkID{
					NetworkID: "lan-" + shortStableHex(key),
					Scope:     scope,
				})
			case *net.IPAddr:
				if !selection.allowsIP(value.IP, isLoopback) {
					continue
				}
			}
		}
	}
	slices.SortFunc(candidates, func(left, right DerivedNetworkID) int {
		if left.Scope < right.Scope {
			return -1
		}
		if left.Scope > right.Scope {
			return 1
		}
		if left.NetworkID < right.NetworkID {
			return -1
		}
		if left.NetworkID > right.NetworkID {
			return 1
		}
		return 0
	})
	return candidates, nil
}

// ListReachabilityScopes derives local subnet scopes from local interfaces.
func ListReachabilityScopes(selection AddressSelection) ([]string, error) {
	candidates, err := ListNetworkIDCandidates(selection)
	if err != nil {
		return nil, err
	}
	scopes := make([]string, 0, len(candidates))
	for _, candidate := range candidates {
		scopes = append(scopes, candidate.Scope)
	}
	return dedupeStrings(scopes), nil
}

// ResolveNetworkIDWithSelection derives one local discovery domain identifier.
//
// It prefers a single private IPv4 subnet when multiple candidates exist.
func ResolveNetworkIDWithSelection(selection AddressSelection) (string, error) {
	candidates, err := ListNetworkIDCandidates(selection)
	if err != nil {
		return "", err
	}
	if len(candidates) == 0 {
		return "", fmt.Errorf("failed to derive network_id: no eligible local network prefix found")
	}
	if len(candidates) == 1 {
		return candidates[0].NetworkID, nil
	}
	ipv4Candidates := make([]DerivedNetworkID, 0)
	for _, candidate := range candidates {
		if strings.Contains(candidate.Scope, ".") {
			ipv4Candidates = append(ipv4Candidates, candidate)
		}
	}
	if len(ipv4Candidates) == 1 {
		return ipv4Candidates[0].NetworkID, nil
	}
	parts := make([]string, 0, len(candidates))
	for _, candidate := range candidates {
		parts = append(parts, candidate.NetworkID+"("+candidate.Scope+")")
	}
	return "", fmt.Errorf(
		"failed to derive network_id: multiple eligible network prefixes found: %s; specify network_id explicitly or narrow interfaces",
		strings.Join(parts, ", "),
	)
}

func dedupeStrings(values []string) []string {
	if len(values) == 0 {
		return nil
	}
	sorted := append([]string{}, values...)
	slices.Sort(sorted)
	return slices.Compact(sorted)
}

func deriveScopeFromIPNet(value *net.IPNet) (string, bool) {
	if ipv4 := value.IP.To4(); ipv4 != nil {
		mask := value.Mask
		if len(mask) != net.IPv4len {
			mask = mask[len(mask)-net.IPv4len:]
		}
		network := ipv4.Mask(mask)
		ones, _ := value.Mask.Size()
		return fmt.Sprintf("%s/%d", network.String(), ones), true
	}
	ipv6 := value.IP.To16()
	if ipv6 == nil {
		return "", false
	}
	network := ipv6.Mask(value.Mask)
	ones, _ := value.Mask.Size()
	return fmt.Sprintf("%s/%d", network.String(), ones), true
}

func shortStableHex(value string) string {
	hasher := fnv.New64a()
	_, _ = hasher.Write([]byte(value))
	var buf [8]byte
	sum := hasher.Sum(buf[:0])
	return hex.EncodeToString(sum)
}
