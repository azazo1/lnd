package main

/*
#include "lnd.h"
#include <stdlib.h>
*/
import "C"

import (
	"fmt"
	"unsafe"
)

func main() {
	serverURL := C.CString("http://127.0.0.1:8765")
	token := C.CString("dev-token")
	networkID := C.CString("office-a")
	service := C.CString("_demo._tcp")
	defer C.free(unsafe.Pointer(serverURL))
	defer C.free(unsafe.Pointer(token))
	defer C.free(unsafe.Pointer(networkID))
	defer C.free(unsafe.Pointer(service))

	client := C.lnd_client_new(serverURL, token)
	if client == nil {
		fmt.Printf("client init failed: %s\n", C.GoString(C.lnd_last_error()))
		return
	}
	defer C.lnd_client_free(client)

	filter := C.lnd_discovery_filter_new(networkID)
	if filter == nil {
		fmt.Printf("filter init failed: %s\n", C.GoString(C.lnd_last_error()))
		return
	}
	defer C.lnd_discovery_filter_free(filter)

	if !bool(C.lnd_discovery_filter_set_service(filter, service)) {
		fmt.Printf("set service failed: %s\n", C.GoString(C.lnd_last_error()))
		return
	}

	json := C.lnd_discover(client, filter)
	if json == nil {
		fmt.Printf("discover failed: %s\n", C.GoString(C.lnd_last_error()))
		return
	}
	defer C.lnd_string_free(json)

	fmt.Println(C.GoString(json))
}
