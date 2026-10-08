// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package budget reserves finite, durable resource upper bounds before an AI
// dispatch. Neither tasks nor provider responses carry game-writing authority.
package budget

import (
	"context"
	"errors"
	"fmt"
	"io"

	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

var ErrPaused = errors.New("AI_BUDGET_PAUSED")

const MaxMetric uint64 = (1 << 53) - 1

type Units = model.Limits
type Caps struct{ Workspace, Room, Session, Seat, Task Units }
type TaskData struct {
	ID      string
	Subject aicontext.SubjectData
	Advice  bool
	State   string
}
type TaskRecord = auth.Secret[TaskData]
type Task struct{ data **taskData }
type taskData struct {
	owner *Service
	value TaskData
}

func (Task) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<private AI budget task>") }
func (Task) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (t Task) state() *taskData {
	if t.data == nil {
		return nil
	}
	return *t.data
}

type ReservationData struct {
	Task                TaskData
	Amount, Spent       Units
	PromptBytes         uint64
	RequestHash, Status string
}
type Reservation = auth.Secret[ReservationData]
type Ticket struct{ data **ticketData }
type ticketData struct {
	owner  *Service
	value  ReservationData
	prompt aicontext.Prompt
}

func (Ticket) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private AI budget reservation>")
}
func (Ticket) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (t Ticket) state() *ticketData {
	if t.data == nil {
		return nil
	}
	return *t.data
}
func (t Ticket) Status() string {
	if t.state() == nil {
		return ""
	}
	return t.state().value.Status
}

type ResponseData struct {
	Advice      checkpoint.Value
	Usage       Units
	Determinate bool
}
type Response = auth.Secret[ResponseData]
type ResultData struct {
	Advice checkpoint.Value
	Usage  Units
}
type Result = auth.Secret[ResultData]
type Provider interface {
	Call(context.Context, aicontext.Prompt, Units) (Response, error)
}

// Storage binds only the existing, live cookie-bound core transaction. Every
// level's counter, task, reservation and pause are committed atomically.
type Storage interface {
	Bind(core.Transaction) (Transaction, error)
}
type Transaction interface {
	Task(context.Context, aicontext.SubjectData, string) (TaskRecord, error)
	InsertTask(context.Context, TaskRecord) error
	Reservation(context.Context, TaskRecord) (Reservation, error)
	Reserve(context.Context, Reservation, Caps) (Reservation, error)
	Dispatch(context.Context, Reservation) error
	Settle(context.Context, Reservation, Units, bool) (Reservation, error)
	Paused(context.Context, aicontext.SubjectData) (bool, error)
}

func values(v Units) [8]uint64 {
	return [8]uint64{v.Calls, v.Tokens, v.CostMicros, v.LatencyMillis, v.Tools, v.Subagents, v.ContextBytes, v.LocalComputeMillis}
}
func units(v [8]uint64) Units {
	return Units{Calls: v[0], Tokens: v[1], CostMicros: v[2], LatencyMillis: v[3], Tools: v[4], Subagents: v[5], ContextBytes: v[6], LocalComputeMillis: v[7]}
}
func Valid(v Units) bool {
	for _, n := range values(v) {
		if n > MaxMetric {
			return false
		}
	}
	return true
}
func Fits(v, cap Units) bool {
	if !Valid(v) || !Valid(cap) {
		return false
	}
	a, b := values(v), values(cap)
	for i := range a {
		if a[i] > b[i] {
			return false
		}
	}
	return true
}
func Add(a, b Units) (Units, error) {
	if !Valid(a) || !Valid(b) {
		return Units{}, auth.ErrInvalid
	}
	x, y := values(a), values(b)
	for i := range x {
		if y[i] > MaxMetric-x[i] {
			return Units{}, ErrPaused
		}
		x[i] += y[i]
	}
	return units(x), nil
}
func Sub(a, b Units) (Units, error) {
	if !Fits(b, a) {
		return Units{}, auth.ErrDenied
	}
	x, y := values(a), values(b)
	for i := range x {
		x[i] -= y[i]
	}
	return units(x), nil
}
func ValidCaps(c Caps) bool {
	return Valid(c.Workspace) && Valid(c.Room) && Valid(c.Session) && Valid(c.Seat) && Valid(c.Task)
}

// CanReserve treats already held, uncertain consumption as spent for every
// dimension. Zero grants and integer overflow always reject the reservation.
func CanReserve(used, held, amount, cap Units) bool {
	a, e := Add(used, held)
	if e != nil {
		return false
	}
	a, e = Add(a, amount)
	return e == nil && Fits(a, cap)
}
func safe(e error) error {
	if errors.Is(e, ErrPaused) {
		return ErrPaused
	}
	return auth.SafeError(e)
}
