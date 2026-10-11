// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package object

import "context"

// Store is the private immutable object boundary. Keys prove integrity only;
// workspace access is still checked by the installation repository and Reader.
// The deployment owner, rather than consumers, owns the backend lifetime.
type Store interface {
	Put(context.Context, []byte) (string, error)
	Read(context.Context, string) ([]byte, error)
	Verify(context.Context, string) error
}

var _ Store = (*Directory)(nil)
