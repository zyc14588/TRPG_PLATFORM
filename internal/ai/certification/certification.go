// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package certification binds reviewed server probes to exact model tuples.
// A browser selection or a provider's own claim cannot mint a qualification.
package certification

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"slices"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	store "github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

func Hash(v any) string {
	b, _ := json.Marshal(v)
	h := sha256.Sum256(b)
	return hex.EncodeToString(h[:])
}

type Registry struct{ data **registryData }
type registryData struct {
	records map[string]model.Certification
}

func (Registry) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<server model qualification registry>")
}
func (Registry) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (r *Registry) state() *registryData {
	if r == nil || r.data == nil {
		return nil
	}
	return *r.data
}
func New(records []model.Certification) (*Registry, error) {
	if len(records) < 1 || len(records) > 128 {
		return nil, auth.ErrInvalid
	}
	d := &registryData{records: map[string]model.Certification{}}
	for _, record := range records {
		c := record.StorageValue()
		key := c.WorkspaceID + "/" + c.ID
		if !store.ValidID(c.WorkspaceID) || !store.ValidID(c.ID) || c.Level < 1 || c.Level > 4 || c.ExpiresAt.IsZero() || len(c.EvidenceHash) != 64 || len(c.Games) < 1 || len(c.Games) > 128 {
			return nil, auth.ErrInvalid
		}
		if _, ok := d.records[key]; ok {
			return nil, auth.ErrInvalid
		}
		d.records[key] = model.NewCertification(c)
	}
	return &Registry{data: &d}, nil
}
func (r *Registry) Bound(workspace string, b model.CertificateBinding, graph string, caps []string, minLevel int, now time.Time) (model.Certification, error) {
	if r.state() == nil || !checkpoint.IsDigest(graph) || minLevel < 1 || minLevel > 4 || now.IsZero() {
		return model.Certification{}, auth.ErrDenied
	}
	record, ok := r.state().records[workspace+"/"+b.ID]
	c := record.StorageValue()
	if !ok || Hash(c) != b.Hash || !now.Before(c.ExpiresAt) || c.Level < minLevel {
		return model.Certification{}, auth.ErrDenied
	}
	for _, g := range c.Games {
		if g.GraphHash == graph && g.TestVersion == c.Tuple.TestVersion {
			for _, cap := range caps {
				if !slices.Contains(c.Capabilities, cap) || !slices.Contains(g.Capabilities, cap) {
					return model.Certification{}, auth.ErrDenied
				}
			}
			return model.NewCertification(c), nil
		}
	}
	return model.Certification{}, auth.ErrDenied
}

// Probe is trusted operator composition that performs and validates the
// observed response. It returns bytes for evidence hashing, never a claimed
// MC level. Cases are fixed here and failures never create a certificate.
type Probe func(context.Context, model.Tuple, string) ([]byte, error)
type Request struct {
	ID, WorkspaceID, GraphHash string
	Tuple                      model.Tuple
	Level                      int
	ExpiresAt                  time.Time
}

var probeSlots = make(chan struct{}, 4)

func runProbe(ctx context.Context, tuple model.Tuple, name string, probe Probe) ([]byte, error) {
	select {
	case probeSlots <- struct{}{}:
	default:
		return nil, auth.ErrUnavailable
	}
	type result struct {
		raw []byte
		err error
	}
	reply := make(chan result)
	go func() {
		var r result
		defer func() {
			<-probeSlots
			if recover() != nil {
				r.err = auth.ErrUnavailable
			}
			select {
			case reply <- r:
			case <-ctx.Done():
				clear(r.raw)
			}
		}()
		r.raw, r.err = probe(ctx, tuple, name)
	}()
	select {
	case <-ctx.Done():
		return nil, auth.ErrUnavailable
	case r := <-reply:
		if ctx.Err() != nil {
			clear(r.raw)
			return nil, auth.ErrUnavailable
		}
		return r.raw, r.err
	}
}
func Certify(ctx context.Context, r Request, probe Probe) (certificate model.Certification, err error) {
	defer func() {
		if recover() != nil {
			certificate = model.Certification{}
			err = auth.ErrUnavailable
		}
	}()
	if ctx == nil || ctx.Err() != nil || probe == nil || !store.ValidID(r.ID) || !store.ValidID(r.WorkspaceID) || !checkpoint.IsDigest(r.GraphHash) || r.Level < 1 || r.Level > 4 || !store.ValidID(r.Tuple.TestVersion) || !store.ValidID(r.Tuple.PromptTemplate) || r.Tuple.Model == "" || r.Tuple.Endpoint == "" || r.Tuple.Adapter != "openai-compatible" || !time.Now().Before(r.ExpiresAt) || r.ExpiresAt.Sub(time.Now()) > 24*time.Hour {
		return certificate, auth.ErrInvalid
	}
	cases := []string{"narrative", "game-contract"}
	caps := []string{}
	if r.Level >= 2 {
		if r.Tuple.ToolMode != "structured" {
			return certificate, auth.ErrDenied
		}
		cases = append(cases, "structured-action", "malformed-denied")
		caps = append(caps, "structured-actions")
	}
	if r.Level >= 3 {
		cases = append(cases, "seat-isolation", "authority-denied")
		caps = append(caps, "ai-player")
	}
	if r.Level >= 4 {
		cases = append(cases, "host-advice-only")
		caps = append(caps, "ai-host")
	}
	evidence := []struct{ Case, Digest string }{}
	for _, name := range cases {
		deadline, cancel := context.WithTimeout(ctx, time.Second)
		raw, e := runProbe(deadline, r.Tuple, name, probe)
		expired := deadline.Err() != nil
		cancel()
		if e != nil || expired || len(raw) < 1 || len(raw) > 64<<10 {
			clear(raw)
			return certificate, auth.ErrDenied
		}
		h := sha256.Sum256(raw)
		evidence = append(evidence, struct{ Case, Digest string }{name, hex.EncodeToString(h[:])})
		clear(raw)
	}
	hash := Hash(struct {
		Tuple    model.Tuple
		Graph    string
		Evidence any
	}{r.Tuple, r.GraphHash, evidence})
	c := model.CertificationData{ID: r.ID, WorkspaceID: r.WorkspaceID, Tuple: r.Tuple, Level: r.Level, Capabilities: caps, EvidenceHash: hash, ExpiresAt: r.ExpiresAt.UTC(), Games: []model.GameEvidence{{GraphHash: r.GraphHash, TestVersion: r.Tuple.TestVersion, Capabilities: slices.Clone(caps), EvidenceHash: Hash(evidence)}}}
	return model.NewCertification(c), nil
}
