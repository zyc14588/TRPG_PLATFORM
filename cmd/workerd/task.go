// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package main

import (
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

// The startup batch supplies the approved provider, credentials and lifetime.
// This factory starts no listener and grants no direct game repository access.
func taskWorker(options task.WorkerOptions, completions *session.Continuations) (*task.Runtime, error) {
	if completions == nil {
		return nil, task.ErrInvalid
	}
	options.Post = completions.Post
	return task.NewWorker(options)
}
