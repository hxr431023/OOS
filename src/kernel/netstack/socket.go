// Package netstack implements the Hybrid kernel's network protocol stack
// in Go (per net.md). It runs as a userland network service and talks to
// the kernel's socket IO-stream objects through the .api bridge.
package netstack

// SocketType mirrors net.md /object/net/sock socket types.
type SocketType int

const (
	TCP SocketType = iota
	UDP
	RAW
	Tap // virtual NIC backing for vm.md
)

func (t SocketType) String() string {
	switch t {
	case TCP:
		return "tcp"
	case UDP:
		return "udp"
	case RAW:
		return "raw"
	case Tap:
		return "tap"
	}
	return "?"
}

// State is the TCP state machine states (RFC 793).
type State int

const (
	Closed State = iota
	Listen
	SynSent
	SynReceived
	Established
	FinWait1
	FinWait2
	CloseWait
	Closing
	LastAck
	TimeWait
)

func (s State) String() string {
	switch s {
	case Closed:
		return "closed"
	case Listen:
		return "listening"
	case SynSent:
		return "syn_sent"
	case SynReceived:
		return "syn_received"
	case Established:
		return "established"
	case FinWait1:
		return "fin_wait1"
	case FinWait2:
		return "fin_wait2"
	case CloseWait:
		return "close_wait"
	case Closing:
		return "closing"
	case LastAck:
		return "last_ack"
	case TimeWait:
		return "time_wait"
	}
	return "?"
}

// Socket is an IO-stream kernel object: /object/net/sock/<sock_id>
// (net.md §3). It carries control + rx/tx queue entries.
type Socket struct {
	ID         uint64
	Type       SocketType
	State      State
	LocalIP    [4]byte
	LocalPort  uint16
	RemoteIP   [4]byte
	RemotePort uint16
	RxBuf      []byte // rx_queue backed buffer
	TxBuf      []byte // tx_queue backed buffer
	OwnerPID   uint64
}

// NewSocket creates a socket object in Closed state.
func NewSocket(id uint64, t SocketType, owner uint64) *Socket {
	return &Socket{ID: id, Type: t, State: Closed, OwnerPID: owner}
}
