package main

import (
	"context"
	"fmt"
	"log"
	"time"

	lnd "github.com/azazo1/lnd/bindings/go"
)

func main() {
	client := lnd.NewClient("http://127.0.0.1:8765", "dev-token", lnd.WithTimeout(5*time.Second))
	networkID, err := client.ResolveNetworkID()
	if err != nil {
		log.Fatal(err)
	}
	scopes, err := client.ListReachabilityScopes()
	if err != nil {
		log.Fatal(err)
	}
	filter := lnd.NewDiscoveryFilter().WithNetworkID(networkID).WithService("_demo._tcp").AddTag("stable")
	for _, scope := range scopes {
		filter = filter.AddReachabilityScope(scope)
	}
	nodes, err := client.Discover(
		context.Background(),
		filter,
	)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(nodes)
}
