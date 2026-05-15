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
	nodes, err := client.Discover(
		context.Background(),
		lnd.NewDiscoveryFilter(networkID).WithService("_demo._tcp").AddTag("stable"),
	)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(nodes)
}
