// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package outbox creates delivery data only from an authoritative committed
// receipt. The corresponding notification intent is persisted in that commit.
package outbox

import (
	"errors"
	"math"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

var ErrNotification = errors.New("SESSION_NOTIFICATION_REJECTED")

type Notification struct {
	Binding         data.Binding
	Version, Cursor uint64
	Events          []data.JournalEvent
}

func FromCommitted(r data.Receipt) (Notification, error) {
	n := Notification{Binding: r.Header.Binding, Version: r.Version, Cursor: r.Cursor}
	if r.Replayed || r.Header.ReadOnly || !store.ValidID(r.Header.CommandID) || !store.ValidID(r.Header.Principal) || r.Header.ExpectedVersion == 0 || r.Version >= math.MaxInt64 || len(r.Events) == 0 || !store.ValidID(n.Binding.Workspace) || !store.ValidID(n.Binding.Session) || !checkpoint.IsDigest(n.Binding.GraphHash) || !checkpoint.IsDigest(r.Header.Fingerprint) || r.Version != r.Header.ExpectedVersion+1 || len(r.Events) > 64 || r.Cursor < uint64(len(r.Events)) {
		return Notification{}, ErrNotification
	}
	for j, e := range r.Events {
		if !store.ValidID(e.ID) || len(e.Type) == 0 || len(e.Type) > 256 || checkpoint.Validate(e.Payload) != nil {
			return Notification{}, ErrNotification
		}
		n.Events = append(n.Events, data.JournalEvent{Sequence: r.Cursor - uint64(len(r.Events)) + uint64(j) + 1, Version: r.Version, CommandID: r.Header.CommandID, Event: e})
	}
	return n, nil
}
