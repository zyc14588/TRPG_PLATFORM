//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package platform_session_test

import (
	"bytes"
	"context"
	"crypto/rand"
	"encoding/json"
	"math/big"
	"sync"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	platformsession "github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

type nativePolicy struct {
	mu                                sync.Mutex
	private, commands, points, export bool
	inputs                            int
}

func (p *nativePolicy) Current(ctx context.Context, a launch.SessionAccess) (platformsession.SeatPolicy, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	v := a.StorageValue()
	fields := []string{"counter", "pending_action"}
	if v.Seat == "gm" && p.private {
		fields = append(fields, "secret")
	}
	view := command.ViewPolicy{ViewFields: fields, EventFields: map[string][]string{PackageID + "/change": {"counter"}}, ScalarResult: true}
	if v.Seat == "gm" && p.private {
		view.EventFields[PackageID+"/change"] = []string{"counter", "secret"}
	}
	commands := map[string]func(checkpoint.Value) error{}
	validate := func(v checkpoint.Value) error {
		if v.Kind != "table" || len(v.Table) != 1 || v.Table["delta"].Kind != "integer" {
			return command.ErrEnvelope
		}
		return nil
	}
	if p.commands {
		commands["increment"] = validate
		commands["fail"] = validate
		if v.Host {
			commands["end"] = func(checkpoint.Value) error { return nil }
		}
	}
	exports := map[platformsession.ExportKind]command.ViewPolicy{}
	if p.export {
		exports[platformsession.Public] = command.ViewPolicy{ViewFields: []string{"counter"}, EventFields: map[string][]string{PackageID + "/change": {"counter"}}}
		exports[platformsession.Personal] = view
		if v.Host {
			exports[platformsession.Host] = view
		}
		if v.Administrator {
			exports[platformsession.Administrator] = view
		}
	}
	inputs := func(ctx context.Context, e command.Envelope) (command.NativeInputs, error) {
		if ctx.Err() != nil {
			return command.NativeInputs{}, auth.ErrUnavailable
		}
		n, err := rand.Int(rand.Reader, big.NewInt(10))
		if err != nil {
			return command.NativeInputs{}, auth.ErrUnavailable
		}
		p.mu.Lock()
		p.inputs++
		p.mu.Unlock()
		return command.NativeInputs{Time: time.Now().UnixMilli(), Random: []int64{n.Int64()}}, nil
	}
	return platformsession.SeatPolicy{Commands: commands, View: view, Inputs: inputs, Exports: exports, RecoveryPoint: p.points}, nil
}

type nativeFixture struct {
	*launchFixture
	native                     *platformsession.Service
	sessions                   *postgres.PlatformSessionStorage
	policy                     *nativePolicy
	room, hostPart, playerPart string
	player                     actor
	launched                   launch.Session
}

func newNativeFixture(t *testing.T, guest bool) *nativeFixture {
	t.Helper()
	l := newLaunchFixture(t, nil)
	n := &nativeFixture{launchFixture: l, policy: &nativePolicy{private: true, commands: true, points: true, export: true}}
	n.room, n.hostPart = l.hostRoom(t)
	inv := l.invite(t, n.room, false, 4)
	if guest {
		n.player, _, _ = l.guest(t, n.room, inv)
		var partID string
		need(t, l.r.Transact(l.ctx, func(tx auth.Transaction) error {
			rt, e := l.store.Bind(tx.Core())
			if e != nil {
				return e
			}
			ps, e := rt.Participants(l.ctx, scopeFor(n))
			if e != nil {
				return e
			}
			for _, p := range ps {
				v := p.StorageValue()
				if v.GuestID != "" {
					partID = v.ID
				}
			}
			return nil
		}))
		n.playerPart = partID
	} else {
		n.player = l.account(t)
		n.playerPart = l.join(t, n.player, inv)["participant_id"].(string)
	}
	prep, e := l.service.Configure(l.ctx, l.caller(l.owner, "native-configure"), auth.RoomSecret(launch.ConfigureData{WorkspaceID: l.w, RoomID: n.room, ConfigurationID: "minimal", Slots: []launch.Slot{{ID: "gm", Mode: "human", ParticipantID: n.hostPart}, {ID: "player", Mode: "human", ParticipantID: n.playerPart}}}))
	need(t, e)
	l.ready(t, n.room, prep)
	need(t, l.acknowledge(t, n.player, n.room, prep.StorageValue().Revision, true, true, true, nil, "player-ready"))
	n.launched, e = l.start(t, l.owner, n.room, prep.StorageValue().Revision, "native-launch")
	need(t, e)
	n.sessions, e = postgres.NewPlatformSessionStorage(l.r)
	need(t, e)
	n.compose(t)
	return n
}
func scopeFor(n *nativeFixture) core.Scope {
	return core.Scope{WorkspaceID: n.w, RoomID: n.room, GameID: "game"}
}
func (n *nativeFixture) compose(t *testing.T) {
	t.Helper()
	var e error
	n.native, e = platformsession.New(platformsession.Options{Launch: n.service, Storage: n.sessions, Policies: n.policy})
	need(t, e)
}
func (n *nativeFixture) connect(t *testing.T, a actor, after uint64) *platformsession.Connection {
	t.Helper()
	c, e := n.native.Connect(n.ctx, n.caller(a, "native-connection"), n.w, n.room, after)
	need(t, e)
	t.Cleanup(c.Close)
	return c
}
func (n *nativeFixture) envelope(id, seat, typ string, version uint64) command.Envelope {
	return command.Envelope{CommandID: id, SessionID: n.launched.StorageValue().Binding.Session, ExpectedStateVersion: version, SeatID: seat, Type: typ, Payload: checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(1)}), CorrelationID: "native-" + id}
}
func nextNative(t *testing.T, n *nativeFixture, c *platformsession.Connection) platformsession.Frame {
	t.Helper()
	ctx, cancel := context.WithTimeout(n.ctx, 5*time.Second)
	defer cancel()
	v, e := c.Next(ctx)
	need(t, e)
	return v
}
func checkPrivate(t *testing.T, value any, expected bool) {
	t.Helper()
	b, e := json.Marshal(value)
	if e != nil {
		t.Fatal("filtered output not encodable")
	}
	defer clear(b)
	has := bytes.Contains(b, []byte(PrivateValue))
	if has != expected {
		t.Fatal("current seat view isolation mismatch; output withheld")
	}
}
