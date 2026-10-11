// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package m2

import (
	"context"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestBrokerAuthenticatedClaimsBindEpochDeadlineAndOneAttempt(t *testing.T) {
	dir := privateDirectory(t)
	f := pkiFixture(t, dir, "platformd", "workerd")
	c := Config{TLS: f["platformd"], WorkerSocket: filepath.Join(dir, "worker.sock"), PeerUID: uint32(os.Getuid())}
	broker, e := NewBroker("fixed-test-binding")
	if e != nil {
		t.Fatal(e)
	}
	defer broker.Close()
	tc, e := TLSConfig(c.TLS, "workerd", true)
	if e != nil {
		t.Fatal(e)
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	service, e := ServeUnix(ctx, c.WorkerSocket, tc, c.PeerUID, broker.Handler())
	if e != nil {
		t.Fatal(e)
	}
	defer func() {
		stop, c := context.WithTimeout(context.Background(), time.Second)
		defer c()
		service.Close(stop)
	}()
	c.TLS = f["workerd"]
	tc, e = TLSConfig(c.TLS, "platformd", false)
	if e != nil {
		t.Fatal(e)
	}
	client, e := UnixClient(c.WorkerSocket, tc, c.PeerUID)
	if e != nil {
		t.Fatal(e)
	}
	defer client.CloseIdleConnections()
	epoch, e := nonce()
	if e != nil {
		t.Fatal(e)
	}
	w := &WorkerClient{client: client, epoch: epoch, binding: broker.binding}
	if w.Probe(ctx) == nil {
		t.Fatal("health registered a worker")
	}
	if e := w.Ready(ctx); e != nil {
		t.Fatal(e)
	}
	handle, e := nonce()
	if e != nil {
		t.Fatal(e)
	}
	attempt, cancelAttempt := context.WithTimeout(ctx, time.Second)
	defer cancelAttempt()
	x := &providerAttempt{ctx: attempt, wire: workerHandle{Handle: handle, BrokerEpoch: broker.epoch, Binding: broker.binding, Deadline: time.Now().Add(time.Second).UTC()}, state: "queued", done: make(chan struct{})}
	broker.calls[handle] = x
	broker.queue <- handle
	claim, e := w.operation(ctx, "claim", workerRequest{})
	if e != nil || claim.State != "claimed" || claim.Handle == nil || claim.Handle.WorkerEpoch != w.epoch {
		t.Fatal("claim binding failed", e)
	}
	bad := *claim.Handle
	bad.BrokerEpoch = "stale"
	if _, e := w.operation(ctx, "begin", workerRequest{Handle: &bad}); e == nil {
		t.Fatal("cross-epoch begin accepted")
	}
	// No trusted live guard exists on this synthetic attempt: no bytes release.
	if _, e := w.operation(ctx, "begin", workerRequest{Handle: claim.Handle}); e == nil {
		t.Fatal("begin without current trusted guard accepted")
	}
	state, e := w.operation(ctx, "status", workerRequest{Handle: claim.Handle})
	if e != nil || state.State != "unknown" || state.Payload != nil {
		t.Fatal("unproven begin became replayable", e)
	}
	state, e = w.operation(ctx, "begin", workerRequest{Handle: claim.Handle})
	if e != nil || state.State != "unknown" || state.Payload != nil {
		t.Fatal("duplicate begin redelivered")
	}
	claim, e = w.operation(ctx, "claim", workerRequest{})
	if e != nil || claim.State != "idle" {
		t.Fatal("unknown attempt was requeued", e)
	}
}
