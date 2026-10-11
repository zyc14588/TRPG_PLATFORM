// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package m2

import (
	"bytes"
	"context"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"sync/atomic"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
)

func TestObjectServiceUsesImmutableDirectoryAndNeverFallsBack(t *testing.T) {
	dir := privateDirectory(t)
	f := pkiFixture(t, dir, "platformd", "object-storage")
	c := Config{TLS: f["object-storage"], ObjectRoot: filepath.Join(dir, "objects"), ObjectSocket: filepath.Join(dir, "objects.sock"), PeerUID: uint32(os.Getuid())}
	if e := os.Mkdir(c.ObjectRoot, 0700); e != nil {
		t.Fatal(e)
	}
	o, e := NewObjects(c.ObjectRoot)
	if e != nil {
		t.Fatal(e)
	}
	defer o.Close()
	tc, e := TLSConfig(c.TLS, "platformd", true)
	if e != nil {
		t.Fatal(e)
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	s, e := ServeUnix(ctx, c.ObjectSocket, tc, c.PeerUID, o.Handler())
	if e != nil {
		t.Fatal(e)
	}
	defer func() { stop, c := context.WithTimeout(context.Background(), time.Second); defer c(); s.Close(stop) }()
	c.TLS = f["platformd"]
	client, e := NewObjectClient(c)
	if e != nil {
		t.Fatal(e)
	}
	defer client.Close()
	body := []byte("synthetic immutable package content")
	key, e := client.Put(ctx, body)
	if e != nil || key != object.Hash(body) {
		t.Fatal("immutable put failed", e)
	}
	again, e := client.Put(ctx, bytes.Clone(body))
	if e != nil || again != key {
		t.Fatal("duplicate content changed", e)
	}
	got, e := client.Read(ctx, key)
	if e != nil || !bytes.Equal(got, body) || client.Verify(ctx, key) != nil {
		t.Fatal("authenticated immutable read failed", e)
	}
	// Corruption occurs only in this test-owned object's private directory.
	paths, e := filepath.Glob(filepath.Join(c.ObjectRoot, "*"))
	if e != nil || len(paths) != 1 {
		t.Fatal("CAS inventory unavailable", e, paths)
	}
	if e = os.Chmod(paths[0], 0600); e != nil {
		t.Fatal(e)
	}
	if e = os.WriteFile(paths[0], []byte("corruption"), 0600); e != nil {
		t.Fatal(e)
	}
	if _, e := client.Read(ctx, key); e == nil {
		t.Fatal("corrupt content read accepted")
	}
	if client.Verify(ctx, key) == nil {
		t.Fatal("corrupt content verified")
	}
	stop, done := context.WithTimeout(ctx, time.Second)
	defer done()
	s.Close(stop)
	if _, e := client.Put(ctx, body); e == nil {
		t.Fatal("object daemon loss silently fell back")
	}
}

func TestObjectClientSerializesFourAdmissionsAndCancelsQueuedWithoutIO(t *testing.T) {
	dir := privateDirectory(t)
	f := pkiFixture(t, dir, "platformd", "object-storage")
	c := Config{TLS: f["object-storage"], ObjectRoot: filepath.Join(dir, "objects"), ObjectSocket: filepath.Join(dir, "objects.sock"), PeerUID: uint32(os.Getuid())}
	if e := os.Mkdir(c.ObjectRoot, 0700); e != nil {
		t.Fatal(e)
	}
	o, e := NewObjects(c.ObjectRoot)
	if e != nil {
		t.Fatal(e)
	}
	defer o.Close()
	tc, e := TLSConfig(c.TLS, "platformd", true)
	if e != nil {
		t.Fatal(e)
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	entered, release := make(chan struct{}), make(chan struct{})
	var reads, active, peak atomic.Int64
	h := o.Handler()
	s, e := ServeUnix(ctx, c.ObjectSocket, tc, c.PeerUID, http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/read" {
			n := active.Add(1)
			defer active.Add(-1)
			for old := peak.Load(); n > old && !peak.CompareAndSwap(old, n); old = peak.Load() {
			}
			if reads.Add(1) == 1 {
				close(entered)
				select {
				case <-release:
				case <-r.Context().Done():
					return
				}
			}
		}
		h.ServeHTTP(w, r)
	}))
	if e != nil {
		t.Fatal(e)
	}
	defer func() {
		stop, cancel := context.WithTimeout(context.Background(), time.Second)
		defer cancel()
		s.Close(stop)
	}()
	c.TLS = f["platformd"]
	client, e := NewObjectClient(c)
	if e != nil {
		t.Fatal(e)
	}
	defer client.Close()
	body := []byte("synthetic bounded ordered immutable operations")
	key, e := client.Put(ctx, body)
	if e != nil {
		t.Fatal(e)
	}
	type result struct {
		id  int
		err error
	}
	finished := make(chan result, 4)
	read := func(ctx context.Context, id int) {
		b, e := client.Read(ctx, key)
		if e == nil && !bytes.Equal(b, body) {
			e = object.ErrIntegrity
		}
		clear(b)
		finished <- result{id, e}
	}
	go read(ctx, 0)
	select {
	case <-entered:
	case <-time.After(2 * time.Second):
		t.Fatal("real byte operation not observed")
	}
	waiting, stopWaiting := context.WithCancel(ctx)
	defer stopWaiting()
	go read(waiting, 1)
	go read(ctx, 2)
	go read(ctx, 3)
	deadline := time.Now().Add(2 * time.Second)
	for len(client.admitted) != 4 {
		if time.Now().After(deadline) {
			t.Fatal("four bounded calls not admitted")
		}
		time.Sleep(time.Millisecond)
	}
	if _, e := client.Read(ctx, key); e == nil || reads.Load() != 1 {
		t.Fatal("fifth admission performed I/O")
	}
	stopWaiting()
	select {
	case r := <-finished:
		if r.id != 1 || r.err == nil || reads.Load() != 1 {
			t.Fatal("queued cancel performed I/O or returned success")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("queued cancellation did not join")
	}
	close(release)
	for range 3 {
		select {
		case r := <-finished:
			if r.err != nil {
				t.Fatal("ordered current operation failed", r.err)
			}
		case <-time.After(3 * time.Second):
			t.Fatal("ordered operation did not finish")
		}
	}
	if reads.Load() != 3 || peak.Load() != 1 || len(client.admitted) != 0 {
		t.Fatal("extra attempt, concurrent byte operation or retained admission")
	}
}

type blockedObjectBody struct {
	entered chan struct{}
	release chan struct{}
	reads   atomic.Uint64
}

func (b *blockedObjectBody) Read(p []byte) (int, error) {
	if b.reads.Add(1) == 1 {
		close(b.entered)
		<-b.release
	}
	return 0, io.EOF
}
func TestObjectServiceBoundsActiveByteOperationsBeforeReading(t *testing.T) {
	s, e := NewObjects(privateDirectory(t))
	if e != nil {
		t.Fatal(e)
	}
	defer s.Close()
	h := s.Handler()
	body := &blockedObjectBody{entered: make(chan struct{}), release: make(chan struct{})}
	r := httptest.NewRequest("POST", "https://object-storage/put", body)
	r.Header.Set("Content-Type", "application/octet-stream")
	finished := make(chan struct{})
	go func() { defer close(finished); h.ServeHTTP(httptest.NewRecorder(), r) }()
	<-body.entered
	rejected := &blockedObjectBody{entered: make(chan struct{}), release: make(chan struct{})}
	r = httptest.NewRequest("POST", "https://object-storage/put", rejected)
	r.Header.Set("Content-Type", "application/octet-stream")
	w := httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != 503 || rejected.reads.Load() != 0 {
		t.Fatal("busy object service consumed another body")
	}
	w = httptest.NewRecorder()
	r = httptest.NewRequest("POST", "https://object-storage/health", bytes.NewReader([]byte("{}")))
	r.Header.Set("Content-Type", "application/json")
	h.ServeHTTP(w, r)
	if w.Code != 200 {
		t.Fatal("bounded byte operation blocked readiness")
	}
	close(body.release)
	<-finished
}

func orderedObjectFixture(t *testing.T, wrapper func(http.Handler) http.Handler) (*ObjectClient, string) {
	t.Helper()
	dir := privateDirectory(t)
	f := pkiFixture(t, dir, "platformd", "object-storage")
	root := filepath.Join(dir, "objects")
	if e := os.Mkdir(root, 0700); e != nil {
		t.Fatal(e)
	}
	o, e := NewObjects(root)
	if e != nil {
		t.Fatal(e)
	}
	t.Cleanup(func() { _ = o.Close() })
	tls, e := TLSConfig(f["object-storage"], "platformd", true)
	if e != nil {
		t.Fatal(e)
	}
	ctx, cancel := context.WithCancel(context.Background())
	t.Cleanup(cancel)
	s, e := ServeUnix(ctx, filepath.Join(dir, "objects.sock"), tls, uint32(os.Getuid()), wrapper(o.Handler()))
	if e != nil {
		t.Fatal(e)
	}
	t.Cleanup(func() {
		stop, done := context.WithTimeout(context.Background(), time.Second)
		defer done()
		_ = s.Close(stop)
	})
	c, e := NewObjectClient(Config{TLS: f["platformd"], ObjectSocket: filepath.Join(dir, "objects.sock"), PeerUID: uint32(os.Getuid())})
	if e != nil {
		t.Fatal(e)
	}
	t.Cleanup(c.Close)
	key, e := c.Put(ctx, []byte("bounded byte operation proof"))
	if e != nil {
		t.Fatal(e)
	}
	return c, key
}

func TestObjectClientQueueAndIOShareEntryBudget(t *testing.T) {
	var reads atomic.Uint64
	client, key := orderedObjectFixture(t, func(h http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if r.URL.Path == "/read" {
				reads.Add(1)
			}
			h.ServeHTTP(w, r)
		})
	})
	// Occupy this private scheduling gate to isolate the queue budget from the
	// original transport timeout. The authenticated service remains available.
	client.active <- struct{}{}
	for _, bound := range []time.Duration{50 * time.Millisecond, 0} {
		ctx := context.Background()
		cancel := func() {}
		if bound != 0 {
			ctx, cancel = context.WithTimeout(ctx, bound)
		}
		start := time.Now()
		_, e := client.Read(ctx, key)
		elapsed := time.Since(start)
		cancel()
		limit := bound
		if limit == 0 {
			limit = 5 * time.Second
		}
		if e == nil || elapsed > limit+500*time.Millisecond || reads.Load() != 0 || len(client.admitted) != 0 {
			t.Fatal("queue extended original deadline/budget, sent bytes or retained admission")
		}
	}
	<-client.active
	cancelled, cancel := context.WithCancel(context.Background())
	cancel()
	for range 128 {
		if _, e := client.Read(cancelled, key); e == nil || reads.Load() != 0 || len(client.active) != 0 || len(client.admitted) != 0 {
			t.Fatal("available active slot admitted canceled request")
		}
	}
	if _, e := client.Read(context.Background(), key); e != nil || reads.Load() != 1 || len(client.active) != 0 || len(client.admitted) != 0 {
		t.Fatal("queue expiration affected subsequent authenticated operation")
	}
}

func TestObjectClientActiveCancelAndResponseEOFFreeSlotsWithoutRetry(t *testing.T) {
	var reads atomic.Uint64
	var mode atomic.Uint32
	entered, joined := make(chan struct{}), make(chan struct{})
	client, key := orderedObjectFixture(t, func(h http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if r.URL.Path == "/read" {
				reads.Add(1)
				switch mode.Load() {
				case 1:
					_, _ = io.Copy(io.Discard, io.LimitReader(r.Body, 1025))
					close(entered)
					<-r.Context().Done()
					close(joined)
					return
				case 2:
					conn, buf, e := w.(http.Hijacker).Hijack()
					if e != nil {
						return
					}
					_, _ = buf.WriteString("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 64\r\n\r\npartial")
					_ = buf.Flush()
					_ = conn.Close()
					return
				}
			}
			h.ServeHTTP(w, r)
		})
	})
	mode.Store(1)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	finished := make(chan error, 1)
	go func() { _, e := client.Read(ctx, key); finished <- e }()
	select {
	case <-entered:
	case <-time.After(2 * time.Second):
		t.Fatal("active service request not observed")
	}
	cancel()
	select {
	case e := <-finished:
		if e == nil {
			t.Fatal("active cancellation returned success")
		}
	case <-time.After(time.Second):
		t.Fatal("active cancellation did not return")
	}
	select {
	case <-joined:
	case <-time.After(time.Second):
		t.Fatal("actual service context not canceled")
	}
	if reads.Load() != 1 || len(client.active) != 0 || len(client.admitted) != 0 {
		t.Fatal("active cancel retried or retained slot")
	}
	mode.Store(2)
	if _, e := client.Read(context.Background(), key); e == nil || reads.Load() != 2 || len(client.active) != 0 || len(client.admitted) != 0 {
		t.Fatal("EOF accepted, retried or retained slot")
	}
	mode.Store(0)
	if _, e := client.Read(context.Background(), key); e != nil || reads.Load() != 3 || len(client.active) != 0 || len(client.admitted) != 0 {
		t.Fatal("EOF/cancel prevented later valid operation")
	}
}
