// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package main

import (
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
	"testing"
)

func TestModelWorkerCannotStartWithoutGatewayAndNativeCompletionAuthority(t *testing.T) {
	if _, e := modelWorker(task.WorkerOptions{}, nil, nil, nil); e != task.ErrInvalid {
		t.Fatal("missing gateway/native authority accepted")
	}
}
