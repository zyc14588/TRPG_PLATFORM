// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package core

import (
	"context"
	"fmt"
	"io"
	"time"
)

type Service struct{ data **serviceData }

type serviceData struct {
	repository Repository
	issuer     *issuerKey
}

func NewService(repository Repository) (*Service, error) {
	if repository == nil {
		return nil, ErrDenied
	}
	d := &serviceData{repository: repository, issuer: &issuerKey{marker: 1}}
	return &Service{data: &d}, nil
}

func (*Service) String() string             { return "<platform service>" }
func (*Service) GoString() string           { return "<platform service>" }
func (*Service) Format(s fmt.State, _ rune) { _, _ = io.WriteString(s, "<platform service>") }

func (s *Service) state() *serviceData {
	if s == nil || s.data == nil || *s.data == nil {
		return nil
	}
	return *s.data
}

func (s *Service) transact(ctx context.Context, f func(Transaction) error) error {
	d := s.state()
	if d == nil || ctx == nil || ctx.Err() != nil {
		return ErrUnavailable
	}
	return SafeError(d.repository.Transact(ctx, f))
}

func (s *Service) identity(actor Actor) (*actorData, error) {
	d := s.state()
	if d == nil || actor.data == nil || *actor.data == nil || (*actor.data).issuer != d.issuer {
		return nil, ErrDenied
	}
	return *actor.data, nil
}

func (s *Service) issue(id string, scope Scope, guest bool) Actor {
	d := &actorData{issuer: s.state().issuer, id: id, scope: scope, guest: guest}
	return Actor{data: &d}
}

// RegisterAccount and DisableAccount are trusted onboarding/security seams,
// not application-user operations. The later authentication adapter must verify
// identity before issuing a handle; knowing an account ID is not authentication.
func (s *Service) RegisterAccount(ctx context.Context, id, displayName string) error {
	if !validID(id) || !validLabel(displayName) {
		return ErrDenied
	}
	return s.transact(ctx, func(tx Transaction) error {
		return tx.InsertAccount(ctx, Account{ID: id, DisplayName: displayName})
	})
}

func (s *Service) DisableAccount(ctx context.Context, id string) error {
	if !validID(id) {
		return ErrDenied
	}
	return s.transact(ctx, func(tx Transaction) error { return tx.DisableAccount(ctx, id) })
}

func (s *Service) AccountActor(ctx context.Context, verifiedAccountID string) (Actor, error) {
	if !validID(verifiedAccountID) {
		return Actor{}, ErrDenied
	}
	err := s.transact(ctx, func(tx Transaction) error {
		_, err := activeAccount(ctx, tx, verifiedAccountID)
		return err
	})
	if err != nil {
		return Actor{}, err
	}
	return s.issue(verifiedAccountID, Scope{}, false), nil
}

func (s *Service) GuestActor(ctx context.Context, verifiedScope Scope, verifiedGuestID string) (Actor, error) {
	if !validScope(verifiedScope) || !validID(verifiedGuestID) {
		return Actor{}, ErrDenied
	}
	err := s.transact(ctx, func(tx Transaction) error {
		if _, err := tx.Workspace(ctx, verifiedScope.WorkspaceID); err != nil {
			return err
		}
		_, err := activeGuest(ctx, tx, verifiedScope, verifiedGuestID)
		return err
	})
	if err != nil {
		return Actor{}, err
	}
	return s.issue(verifiedGuestID, verifiedScope, true), nil
}

func activeAccount(ctx context.Context, tx Transaction, id string) (Account, error) {
	a, err := tx.Account(ctx, id)
	if err != nil {
		return Account{}, err
	}
	if a.Disabled {
		return Account{}, ErrDenied
	}
	return a, nil
}

func activeGuest(ctx context.Context, tx Transaction, scope Scope, id string) (Guest, error) {
	g, err := tx.Guest(ctx, scope, id)
	if err != nil {
		return Guest{}, err
	}
	now, err := tx.Now(ctx)
	if err != nil {
		return Guest{}, err
	}
	if g.Disabled || g.ClaimedAccountID != "" || !now.Before(g.ExpiresAt) {
		return Guest{}, ErrDenied
	}
	return g, nil
}

func member(ctx context.Context, tx Transaction, workspace string, actor *actorData) (Membership, error) {
	if actor.guest {
		return Membership{}, ErrDenied
	}
	if _, err := activeAccount(ctx, tx, actor.id); err != nil {
		return Membership{}, err
	}
	return tx.Membership(ctx, workspace, actor.id)
}

func (s *Service) CreateWorkspace(ctx context.Context, actor Actor, id, name string) error {
	a, err := s.identity(actor)
	if err != nil || a.guest || !validID(id) || !validLabel(name) {
		return ErrDenied
	}
	return s.transact(ctx, func(tx Transaction) error {
		if _, err := activeAccount(ctx, tx, a.id); err != nil {
			return err
		}
		if err := tx.InsertWorkspace(ctx, Workspace{ID: id, Name: name, OwnerID: a.id}); err != nil {
			return err
		}
		return tx.PutMembership(ctx, Membership{WorkspaceID: id, AccountID: a.id, Role: Owner})
	})
}

func (s *Service) Workspace(ctx context.Context, actor Actor, id string) (Workspace, error) {
	a, err := s.identity(actor)
	if err != nil || a.guest || !validID(id) {
		return Workspace{}, ErrDenied
	}
	var result Workspace
	err = s.transact(ctx, func(tx Transaction) error {
		w, err := tx.Workspace(ctx, id)
		if err != nil {
			return err
		}
		m, err := member(ctx, tx, id, a)
		if err != nil {
			return err
		}
		if !workspaceAllowed(m.Role, ReadWorkspace) {
			return ErrDenied
		}
		result = w
		return nil
	})
	if err != nil {
		return Workspace{}, err
	}
	return result, nil
}

func (s *Service) changeMembership(ctx context.Context, actor Actor, workspaceID, accountID string, role Role, remove bool) error {
	a, err := s.identity(actor)
	if err != nil || a.guest || !validID(workspaceID) || !validID(accountID) || (!remove && role != Admin && role != Member) {
		return ErrDenied
	}
	return s.transact(ctx, func(tx Transaction) error {
		w, err := tx.Workspace(ctx, workspaceID)
		if err != nil {
			return err
		}
		m, err := member(ctx, tx, workspaceID, a)
		if err != nil {
			return err
		}
		if !workspaceAllowed(m.Role, ManageWorkspace) || w.OwnerID == accountID {
			return ErrDenied
		}
		if remove {
			if _, err := tx.Account(ctx, accountID); err != nil {
				return err
			}
		} else if _, err := activeAccount(ctx, tx, accountID); err != nil {
			return err
		}
		if m.Role != Owner {
			current, err := tx.Membership(ctx, workspaceID, accountID)
			if err != nil && err != ErrDenied {
				return err
			}
			if role == Admin || current.Role == Admin || current.Role == Owner {
				return ErrDenied
			}
		}
		if remove {
			return tx.DeleteMembership(ctx, workspaceID, accountID)
		}
		return tx.PutMembership(ctx, Membership{WorkspaceID: workspaceID, AccountID: accountID, Role: role})
	})
}

func (s *Service) SetMembership(ctx context.Context, actor Actor, workspaceID, accountID string, role Role) error {
	return s.changeMembership(ctx, actor, workspaceID, accountID, role, false)
}

func (s *Service) RemoveMembership(ctx context.Context, actor Actor, workspaceID, accountID string) error {
	return s.changeMembership(ctx, actor, workspaceID, accountID, "", true)
}

// ProvisionGuest is used only after a room service has verified joining rights.
// This batch persists the restricted identity; it does not create invitations,
// room admission, seats or a claim credential.
func (s *Service) ProvisionGuest(ctx context.Context, manager Actor, scope Scope, id string, expiresAt time.Time) error {
	a, err := s.identity(manager)
	if err != nil || a.guest || !validScope(scope) || !validID(id) {
		return ErrDenied
	}
	return s.transact(ctx, func(tx Transaction) error {
		if _, err := tx.Workspace(ctx, scope.WorkspaceID); err != nil {
			return err
		}
		m, err := member(ctx, tx, scope.WorkspaceID, a)
		if err != nil {
			return err
		}
		now, err := tx.Now(ctx)
		if err != nil {
			return err
		}
		if !workspaceAllowed(m.Role, ManageWorkspace) || !expiresAt.After(now) {
			return ErrDenied
		}
		return tx.InsertGuest(ctx, Guest{Scope: scope, ID: id, ExpiresAt: expiresAt.UTC()})
	})
}

func (s *Service) RevokeGuest(ctx context.Context, manager Actor, scope Scope, id string) error {
	a, err := s.identity(manager)
	if err != nil || a.guest || !validScope(scope) || !validID(id) {
		return ErrDenied
	}
	return s.transact(ctx, func(tx Transaction) error {
		if _, err := tx.Workspace(ctx, scope.WorkspaceID); err != nil {
			return err
		}
		m, err := member(ctx, tx, scope.WorkspaceID, a)
		if err != nil {
			return err
		}
		if !workspaceAllowed(m.Role, ManageWorkspace) {
			return ErrDenied
		}
		return tx.DisableGuest(ctx, scope, id)
	})
}

// ClaimGuest requires both verified identity handles. The scoped participation
// keeps its original ID and boundary; no membership or management grant is made.
func (s *Service) ClaimGuest(ctx context.Context, guestActor, accountActor Actor) (Participation, error) {
	g, ge := s.identity(guestActor)
	a, ae := s.identity(accountActor)
	if ge != nil || ae != nil || !g.guest || a.guest {
		return Participation{}, ErrDenied
	}
	var result Participation
	err := s.transact(ctx, func(tx Transaction) error {
		if _, err := tx.Workspace(ctx, g.scope.WorkspaceID); err != nil {
			return err
		}
		if _, err := activeGuest(ctx, tx, g.scope, g.id); err != nil {
			return err
		}
		if _, err := activeAccount(ctx, tx, a.id); err != nil {
			return err
		}
		if err := tx.ClaimGuest(ctx, g.scope, g.id, a.id); err != nil {
			return err
		}
		result = Participation{Scope: g.scope, GuestID: g.id, AccountID: a.id}
		return nil
	})
	if err != nil {
		return Participation{}, err
	}
	return result, nil
}

func (s *Service) Authorize(ctx context.Context, actor Actor, scope Scope, permission Permission) error {
	a, err := s.identity(actor)
	if err != nil || !validID(scope.WorkspaceID) {
		return ErrDenied
	}
	if a.guest && (scope != a.scope || permission != Participate) {
		return ErrDenied
	}
	return s.transact(ctx, func(tx Transaction) error {
		if _, err := tx.Workspace(ctx, scope.WorkspaceID); err != nil {
			return err
		}
		if a.guest {
			_, err := activeGuest(ctx, tx, a.scope, a.id)
			return err
		}
		if permission == Participate {
			if !validScope(scope) {
				return ErrDenied
			}
			if _, err := activeAccount(ctx, tx, a.id); err != nil {
				return err
			}
			_, err := tx.Participation(ctx, scope, a.id)
			return err
		}
		m, err := member(ctx, tx, scope.WorkspaceID, a)
		if err != nil {
			return err
		}
		if !workspaceAllowed(m.Role, permission) {
			return ErrDenied
		}
		return nil
	})
}
