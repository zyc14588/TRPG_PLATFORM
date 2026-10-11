// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//go:build linux

package m2

import (
	"errors"
	"net"
	"os"
	"syscall"
	"time"
)

func checkPeer(c *net.UnixConn, uid uint32) error {
	raw, e := c.SyscallConn()
	if e != nil {
		return ErrPrivate
	}
	var peer *syscall.Ucred
	var ce error
	if e = raw.Control(func(fd uintptr) { peer, ce = syscall.GetsockoptUcred(int(fd), syscall.SOL_SOCKET, syscall.SO_PEERCRED) }); e != nil || ce != nil || peer == nil || peer.Uid != uid {
		return ErrPrivate
	}
	return nil
}

// The exclusive kernel lock proves the previous service no longer owns this
// socket. Only an unchanged, same-UID, refusing socket may then be removed.
func ownSocket(path string) (*os.File, error) {
	fd, e := syscall.Open(path+".owner", syscall.O_CREAT|syscall.O_RDWR|syscall.O_CLOEXEC|syscall.O_NOFOLLOW, 0600)
	if e != nil {
		return nil, ErrPrivate
	}
	f := os.NewFile(uintptr(fd), path+".owner")
	i, e := f.Stat()
	if e != nil || !i.Mode().IsRegular() || i.Mode().Perm() != 0600 {
		f.Close()
		return nil, ErrPrivate
	}
	st, ok := i.Sys().(*syscall.Stat_t)
	if !ok || st.Uid != uint32(os.Geteuid()) || syscall.Flock(fd, syscall.LOCK_EX|syscall.LOCK_NB) != nil {
		f.Close()
		return nil, ErrPrivate
	}
	before, e := os.Lstat(path)
	if errors.Is(e, os.ErrNotExist) {
		return f, nil
	}
	if e != nil || before.Mode()&os.ModeSocket == 0 {
		releaseSocket(f)
		return nil, ErrPrivate
	}
	st, ok = before.Sys().(*syscall.Stat_t)
	if !ok || st.Uid != uint32(os.Geteuid()) {
		releaseSocket(f)
		return nil, ErrPrivate
	}
	c, e := net.DialTimeout("unix", path, 200*time.Millisecond)
	if e == nil {
		c.Close()
		releaseSocket(f)
		return nil, ErrPrivate
	}
	after, statErr := os.Lstat(path)
	if !errors.Is(e, syscall.ECONNREFUSED) || statErr != nil || !os.SameFile(before, after) || os.Remove(path) != nil {
		releaseSocket(f)
		return nil, ErrPrivate
	}
	return f, nil
}
func releaseSocket(f *os.File) {
	if f != nil {
		_ = syscall.Flock(int(f.Fd()), syscall.LOCK_UN)
		_ = f.Close()
	}
}
