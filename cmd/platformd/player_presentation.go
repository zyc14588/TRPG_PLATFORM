// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package main

import (
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/httpapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
)

// B011 owns daemon startup/listener wiring. This explicit composition starts
// neither a listener nor a session and does not change the current main entry.
func platformPlayersWithPresentation(o platformPlayerOptions, models *model.Service, labels map[string]string, schema []byte) (*platformPlayerComponents, error) {
	if models == nil || o.models != models {
		return nil, auth.ErrInvalid
	}
	c, e := platformPlayers(o)
	if e != nil {
		return nil, e
	}
	fail := func(e error) (*platformPlayerComponents, error) { _ = c.close(); return nil, e }
	source, e := model.NewPresentationSource(models, labels)
	if e != nil {
		return fail(e)
	}
	facade, e := player.NewPresentationService(c.state().players, source, schema)
	if e != nil {
		return fail(e)
	}
	handler, e := httpapi.NewPlayerPresentationHandler(c.state().players, o.rooms, facade, o.origin)
	if e != nil {
		return fail(e)
	}
	c.state().handler = handler
	return c, nil
}
