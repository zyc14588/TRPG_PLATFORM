// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package m2

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"net/http"
	"os"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
)

type ObjectService struct {
	directory *object.Directory
	active    chan struct{}
}

func NewObjects(root string) (*ObjectService, error) {
	if RequireLinux() != nil {
		return nil, ErrConfiguration
	}
	d, e := object.Open(root)
	if e != nil {
		return nil, ErrConfiguration
	}
	// A complete object can be 80 MiB, and Put verifies it before publishing.
	// Bound all byte-bearing operations before reading a request body, so the
	// private listener's connection limit cannot multiply that memory bound.
	return &ObjectService{directory: d, active: make(chan struct{}, 1)}, nil
}
func (s *ObjectService) Close() error { return s.directory.Close() }

type objectKey struct{ Key string }

func (s *ObjectService) Handler() http.Handler {
	m := http.NewServeMux()
	m.HandleFunc("/put", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" || r.URL.RawQuery != "" || r.Header.Get("Content-Type") != "application/octet-stream" {
			privateError(w)
			return
		}
		b, e := io.ReadAll(io.LimitReader(r.Body, object.MaxBytes+1))
		defer clear(b)
		if e != nil || len(b) > object.MaxBytes {
			privateError(w)
			return
		}
		key, e := s.directory.Put(r.Context(), b)
		if e != nil {
			privateError(w)
			return
		}
		privateReply(w, objectKey{key})
	})
	for _, op := range []string{"read", "verify"} {
		m.HandleFunc("/"+op, func(w http.ResponseWriter, r *http.Request) {
			if !privateMethod(w, r) {
				return
			}
			var v objectKey
			if decodeRequest(r.Context(), r.Body, &v, 1024) != nil || !checkpoint.IsDigest(v.Key) {
				privateError(w)
				return
			}
			if op == "verify" {
				if s.directory.Verify(r.Context(), v.Key) != nil {
					privateError(w)
					return
				}
				privateReply(w, v)
				return
			}
			b, e := s.directory.Read(r.Context(), v.Key)
			defer clear(b)
			if e != nil {
				privateError(w)
				return
			}
			w.Header().Set("Content-Type", "application/octet-stream")
			w.Header().Set("Cache-Control", "no-store")
			_, _ = w.Write(b)
		})
	}
	m.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		if !privateMethod(w, r) {
			return
		}
		privateReply(w, struct{ Ready bool }{true})
	})
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/health" {
			m.ServeHTTP(w, r)
			return
		}
		select {
		case s.active <- struct{}{}:
			defer func() { <-s.active }()
			m.ServeHTTP(w, r)
		default:
			fmt.Fprintln(os.Stderr, "M2_OBJECT_OWNER_BUSY")
			privateError(w)
		}
	})
}

type ObjectClient struct {
	client   *http.Client
	admitted chan struct{}
	active   chan struct{}
}

func NewObjectClient(c Config) (*ObjectClient, error) {
	tls, e := TLSConfig(c.TLS, "object-storage", false)
	if e != nil {
		return nil, e
	}
	client, e := UnixClient(c.ObjectSocket, tls, c.PeerUID)
	if e != nil {
		return nil, e
	}
	// One HTTP/1 connection orders the original synchronous store operations.
	// Four admissions bound callers before they wait; no operation is retried.
	transport := client.Transport.(*http.Transport)
	transport.MaxConnsPerHost = 1
	transport.MaxIdleConns = 1
	transport.MaxIdleConnsPerHost = 1
	return &ObjectClient{client: client, admitted: make(chan struct{}, 4), active: make(chan struct{}, 1)}, nil
}

// Queue time is part of the original client budget, not an extra wait before
// starting another complete transport timeout.
func (c *ObjectClient) operation(ctx context.Context) (context.Context, context.CancelFunc, error) {
	if c == nil || c.client == nil || ctx == nil || ctx.Err() != nil {
		return nil, nil, ErrPrivate
	}
	bounded, cancel := context.WithTimeout(ctx, 5*time.Second)
	return bounded, cancel, nil
}
func (c *ObjectClient) enter(ctx context.Context) (func(), error) {
	if c == nil || c.client == nil || ctx == nil || ctx.Err() != nil {
		return nil, ErrPrivate
	}
	select {
	case c.admitted <- struct{}{}:
	default:
		return nil, ErrPrivate
	}
	select {
	case c.active <- struct{}{}:
		if ctx.Err() != nil {
			<-c.active
			<-c.admitted
			return nil, ErrPrivate
		}
		return func() { <-c.active; <-c.admitted }, nil
	case <-ctx.Done():
		<-c.admitted
		return nil, ErrPrivate
	}
}
func (c *ObjectClient) Close() { c.client.CloseIdleConnections() }
func (c *ObjectClient) Ready(ctx context.Context) error {
	r, e := privateRequest(ctx, c.client, "object-storage", "/health", bytes.NewReader([]byte("{}")))
	if e != nil {
		return e
	}
	defer r.Body.Close()
	var v struct{ Ready bool }
	if decodeRequest(ctx, r.Body, &v, 1024) != nil || !v.Ready {
		return ErrPrivate
	}
	return nil
}
func (c *ObjectClient) Put(ctx context.Context, b []byte) (string, error) {
	ctx, cancel, e := c.operation(ctx)
	if e != nil {
		return "", e
	}
	defer cancel()
	if len(b) > object.MaxBytes {
		return "", object.ErrIntegrity
	}
	leave, e := c.enter(ctx)
	if e != nil {
		return "", e
	}
	defer leave()
	r, e := http.NewRequestWithContext(ctx, "POST", "https://object-storage/put", bytes.NewReader(b))
	if e != nil {
		return "", ErrPrivate
	}
	r.Header.Set("Content-Type", "application/octet-stream")
	v, e := c.client.Do(r)
	if e != nil {
		return "", ErrPrivate
	}
	defer v.Body.Close()
	var out objectKey
	if v.StatusCode != 200 || decodeRequest(ctx, v.Body, &out, 1024) != nil || out.Key != object.Hash(b) {
		return "", object.ErrIntegrity
	}
	return out.Key, nil
}
func (c *ObjectClient) Read(ctx context.Context, key string) ([]byte, error) {
	ctx, cancel, e := c.operation(ctx)
	if e != nil {
		return nil, e
	}
	defer cancel()
	if !checkpoint.IsDigest(key) {
		return nil, object.ErrIntegrity
	}
	leave, e := c.enter(ctx)
	if e != nil {
		return nil, e
	}
	defer leave()
	b, _ := encode(objectKey{key})
	r, e := privateRequest(ctx, c.client, "object-storage", "/read", bytes.NewReader(b))
	if e != nil {
		fmt.Fprintln(os.Stderr, "M2_OBJECT_READ_UNAVAILABLE_OR_DENIED")
		return nil, e
	}
	defer r.Body.Close()
	v, e := io.ReadAll(io.LimitReader(r.Body, object.MaxBytes+1))
	if e != nil || len(v) > object.MaxBytes || object.Hash(v) != key {
		clear(v)
		return nil, object.ErrIntegrity
	}
	return v, nil
}
func (c *ObjectClient) Verify(ctx context.Context, key string) error {
	ctx, cancel, e := c.operation(ctx)
	if e != nil {
		return e
	}
	defer cancel()
	if !checkpoint.IsDigest(key) {
		return object.ErrIntegrity
	}
	leave, e := c.enter(ctx)
	if e != nil {
		return e
	}
	defer leave()
	b, _ := encode(objectKey{key})
	r, e := privateRequest(ctx, c.client, "object-storage", "/verify", bytes.NewReader(b))
	if e != nil {
		fmt.Fprintln(os.Stderr, "M2_OBJECT_VERIFY_UNAVAILABLE_OR_DENIED")
		return e
	}
	defer r.Body.Close()
	var v objectKey
	if decodeRequest(ctx, r.Body, &v, 1024) != nil || v.Key != key {
		return object.ErrIntegrity
	}
	return nil
}

var _ object.Store = (*ObjectClient)(nil)
