// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package action separates untrusted proposals from committed, filtered results.
// A provider never receives a command connection, repository or event writer.
package action

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	store "github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

const MaxActionBytes = 16 << 10
const MaxNarrativeBytes = 16 << 10

type ProposalData struct {
	Type            string           `json:"type"`
	ExpectedVersion uint64           `json:"expected_state_version"`
	Payload         checkpoint.Value `json:"payload"`
}
type Proposal = auth.Secret[ProposalData]

// Decode admits only a proposal for the current filtered seat and package
// command allowlist. One local fence removal is the entire format repair;
// unknown fields, duplicate JSON keys and version forgery remain rejected.
func Decode(raw []byte, version uint64, allowed map[string]func(checkpoint.Value) error) (Proposal, error) {
	if len(raw) > MaxActionBytes || version == 0 || version >= 1<<53 || len(allowed) > 32 {
		return Proposal{}, auth.ErrInvalid
	}
	s := strings.TrimSpace(string(raw))
	if strings.HasPrefix(s, "```json\n") && strings.HasSuffix(s, "\n```") {
		s = s[len("```json\n") : len(s)-len("\n```")]
	}
	var p ProposalData
	if checkpoint.StrictDecode([]byte(s), &p, MaxActionBytes) != nil || !store.ValidID(p.Type) || p.ExpectedVersion != version || checkpoint.Validate(p.Payload) != nil {
		return Proposal{}, auth.ErrDenied
	}
	f := allowed[p.Type]
	if f == nil {
		return Proposal{}, auth.ErrDenied
	}
	if e := validate(f, p.Payload); e != nil {
		return Proposal{}, e
	}
	b, e := json.Marshal(p)
	if e != nil {
		return Proposal{}, auth.ErrDenied
	}
	var owned ProposalData
	if json.Unmarshal(b, &owned) != nil {
		return Proposal{}, auth.ErrDenied
	}
	return auth.RoomSecret(owned), nil
}
func validate(f func(checkpoint.Value) error, v checkpoint.Value) (err error) {
	defer func() {
		if recover() != nil {
			err = auth.ErrDenied
		}
	}()
	if f(v) != nil {
		return auth.ErrDenied
	}
	return nil
}

type Commit struct{ data **commitData }
type commitData struct {
	binding         data.Binding
	command         string
	version, cursor uint64
	result          checkpoint.Value
}

func (Commit) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<committed filtered AI result>") }
func (Commit) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (c Commit) state() *commitData {
	if c.data == nil {
		return nil
	}
	return *c.data
}

// Committed is a trusted adapter boundary called after the native authority
// returns its receipt. filtered must be the server's current seat projection;
// the provider does not supply either argument or the projection policy.
func Committed(r data.Receipt, filtered checkpoint.Value) (Commit, error) {
	if r.Header.ReadOnly || !store.ValidID(r.Header.Binding.Workspace) || !store.ValidID(r.Header.Binding.Session) || !checkpoint.IsDigest(r.Header.Binding.GraphHash) || !store.ValidID(r.Header.CommandID) || !store.ValidID(r.Header.Principal) || r.Header.ExpectedVersion == 0 || r.Version != r.Header.ExpectedVersion+1 || r.Version >= 1<<53 || checkpoint.Validate(filtered) != nil {
		return Commit{}, auth.ErrDenied
	}
	b, e := json.Marshal(filtered)
	if e != nil || len(b) > MaxActionBytes {
		return Commit{}, auth.ErrDenied
	}
	var v checkpoint.Value
	if json.Unmarshal(b, &v) != nil {
		return Commit{}, auth.ErrDenied
	}
	d := &commitData{binding: r.Header.Binding, command: r.Header.CommandID, version: r.Version, cursor: r.Cursor, result: v}
	return Commit{data: &d}, nil
}
func (c Commit) Version() uint64 {
	if c.state() == nil {
		return 0
	}
	return c.state().version
}
func (c Commit) Result() (checkpoint.Value, error) {
	if c.state() == nil {
		return checkpoint.Value{}, auth.ErrDenied
	}
	b, _ := json.Marshal(c.state().result)
	var v checkpoint.Value
	if json.Unmarshal(b, &v) != nil {
		return checkpoint.Value{}, auth.ErrDenied
	}
	return v, nil
}
func (c Commit) Template() (string, error) {
	v, e := c.Result()
	if e != nil {
		return "", e
	}
	return ResultTemplate(v)
}

// ResultTemplate formats an already committed server seat projection. It
// grants no authority and accepts no provider-supplied statement of results.
func ResultTemplate(v checkpoint.Value) (string, error) {
	if checkpoint.Validate(v) != nil {
		return "", auth.ErrDenied
	}
	b, e := json.Marshal(v)
	text := "行动已按规则执行。结果：" + string(b)
	if e != nil || len(text) > MaxNarrativeBytes {
		return "", auth.ErrDenied
	}
	return text, nil
}

// AfterCommit may wait only after the caller has completed the authoritative
// transaction. A failed or invalid narrative cannot undo the accepted event.
func AfterCommit(ctx context.Context, c Commit, narrate func(context.Context, Commit) (string, error)) (text string, template bool, err error) {
	if c.state() == nil || ctx == nil || narrate == nil {
		return "", false, auth.ErrDenied
	}
	fallback, e := c.Template()
	if e != nil {
		return "", false, e
	}
	defer func() {
		if recover() != nil {
			text, template, err = fallback, true, nil
		}
	}()
	text, e = narrate(ctx, c)
	if e != nil || ctx.Err() != nil || strings.TrimSpace(text) == "" || len(text) > MaxNarrativeBytes {
		return fallback, true, nil
	}
	return text, false, nil
}
