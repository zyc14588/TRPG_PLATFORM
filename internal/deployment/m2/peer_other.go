// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//go:build !linux

package m2

import (
	"net"
	"os"
)

func checkPeer(*net.UnixConn, uint32) error { return ErrConfiguration }
func ownSocket(string) (*os.File, error)    { return nil, ErrConfiguration }
func releaseSocket(*os.File)                {}
