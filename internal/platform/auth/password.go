// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package auth

import (
	"crypto/rand"
	"crypto/subtle"
	"unicode/utf8"

	"golang.org/x/crypto/argon2"
)

func newPassword(password string) (Password, error) {
	if !utf8.ValidString(password) || utf8.RuneCountInString(password) < 15 || utf8.RuneCountInString(password) > 128 {
		return Password{}, ErrInvalid
	}
	salt := make([]byte, 16)
	if _, e := rand.Read(salt); e != nil {
		return Password{}, ErrUnavailable
	}
	key := argon2.IDKey([]byte(password), salt, 3, 65536, 1, 32)
	return protect(append(salt, key...)), nil
}

func verifyPassword(password string, stored Password) bool {
	data := stored.StorageValue()
	valid := len(data) == 48
	if !valid {
		data = make([]byte, 48)
	}
	// Unknown and disabled accounts execute the same bounded Argon2id path.
	key := argon2.IDKey([]byte(password), data[:16], 3, 65536, 1, 32)
	return subtle.ConstantTimeCompare(key, data[16:]) == 1 && valid
}
