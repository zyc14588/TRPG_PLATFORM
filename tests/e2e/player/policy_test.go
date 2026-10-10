//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player_test

import (
	"context"
	"crypto/rand"
	"math/big"
	"sync"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	platformsession "github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
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
