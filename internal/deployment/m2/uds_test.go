// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package m2

import (
	"bytes"
	"context"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestUDSAuthenticatesRoleAndUIDAndRecoversOnlyAbandonedSocket(t *testing.T) {
	dir := privateDirectory(t)
	f := pkiFixture(t, dir, "platformd", "workerd", "untrusted")
	path := filepath.Join(dir, "service.sock")
	config, e := TLSConfig(f["platformd"], "workerd", true)
	if e != nil {
		t.Fatal(e)
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	s, e := ServeUnix(ctx, path, config, uint32(os.Getuid()), http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { privateReply(w, struct{ Ready bool }{true}) }))
	if e != nil {
		t.Fatal(e)
	}
	closeService := func() {
		stop, c := context.WithTimeout(context.Background(), time.Second)
		defer c()
		if e := s.Close(stop); e != nil {
			t.Error(e)
		}
	}
	defer closeService()
	if _, e := ServeUnix(ctx, path, config, uint32(os.Getuid()), http.NotFoundHandler()); e == nil {
		t.Fatal("live owned socket replaced")
	}
	for _, role := range []string{"workerd", "untrusted"} {
		tc, e := TLSConfig(f[role], "platformd", false)
		if e != nil {
			t.Fatal(e)
		}
		client, e := UnixClient(path, tc, uint32(os.Getuid()))
		if e != nil {
			t.Fatal(e)
		}
		reply, e := privateRequest(ctx, client, "platformd", "/health", bytes.NewReader([]byte("{}")))
		client.CloseIdleConnections()
		if role == "workerd" {
			if e != nil {
				t.Fatal(e)
			}
			reply.Body.Close()
		} else if e == nil {
			reply.Body.Close()
			t.Fatal("unapproved peer certificate accepted")
		}
	}
	tc, _ := TLSConfig(f["workerd"], "platformd", false)
	client, _ := UnixClient(path, tc, uint32(os.Getuid()+1))
	defer client.CloseIdleConnections()
	if r, e := privateRequest(ctx, client, "platformd", "/health", bytes.NewReader([]byte("{}"))); e == nil {
		r.Body.Close()
		t.Fatal("wrong kernel UID accepted")
	}
	closeService()
	abandoned, e := net.ListenUnix("unix", &net.UnixAddr{Name: path, Net: "unix"})
	if e != nil {
		t.Fatal(e)
	}
	abandoned.SetUnlinkOnClose(false)
	abandoned.Close()
	s, e = ServeUnix(ctx, path, config, uint32(os.Getuid()), http.NotFoundHandler())
	if e != nil {
		t.Fatal("owned stale socket could not recover", e)
	}
}
