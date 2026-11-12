package netstack

// Stack is the kernel network protocol stack demux (net.md §1).
// It owns the socket objects and dispatches raw Ethernet frames up the
// protocol layers, routing IP payloads to the matching socket's rx queue.
// Pure logic, no stdlib deps (kernel-mode TinyGo-friendly, slices not maps).
type Stack struct {
	sockets []*Socket
	arp     *ARPTable
	nextID  uint64
	// LocalIP is the primary interface address (from /object/net/dev info).
	LocalIP [4]byte
}

// NewStack creates an empty protocol stack.
func NewStack() *Stack {
	return &Stack{arp: NewARPTable(), nextID: 1}
}

// CreateSocket creates a socket IO-stream object (net.md §3).
func (s *Stack) CreateSocket(t SocketType, owner uint64) *Socket {
	sock := NewSocket(s.nextID, t, owner)
	s.nextID++
	s.sockets = append(s.sockets, sock)
	return sock
}

// RemoveSocket drops a closed socket.
func (s *Stack) RemoveSocket(id uint64) {
	for i := range s.sockets {
		if s.sockets[i].ID == id {
			s.sockets = append(s.sockets[:i], s.sockets[i+1:]...)
			return
		}
	}
}

// findSocket returns the socket matching proto/ports for UDP demux.
func (s *Stack) findSocket(dstPort uint16) *Socket {
	for _, so := range s.sockets {
		if so.Type == UDP && so.LocalPort == dstPort {
			return so
		}
	}
	return nil
}

// HandleIPRoutes routes an IPv4 payload to the right transport and socket.
func (s *Stack) HandleIPRoute(proto uint8, src, dst [4]byte, payload []byte) {
	switch proto {
	case ProtoUDP:
		h, data, ok := ParseUDP(payload)
		if !ok {
			return
		}
		sock := s.findSocket(h.DstPort)
		if sock == nil {
			return
		}
		sock.RemoteIP = src
		sock.RemotePort = h.SrcPort
		// enqueue into rx buffer (rx_queue backed)
		sock.RxBuf = append(sock.RxBuf, data...)
	}
	// TCP handled by TCP state machine (tcp.go) per-socket; demux by 4-tuple
	// would go here when multiple TCP sockets exist.
}

// HandleEthernetFrame processes a raw Ethernet frame (net.md: NIC -> stack).
// ethertype at bytes 12-13; payload after 14-byte header.
func (s *Stack) HandleEthernetFrame(frame []byte) {
	if len(frame) < 14 {
		return
	}
	etherType := uint16(frame[12])<<8 | uint16(frame[13])
	payload := frame[14:]
	switch etherType {
	case 0x0806: // ARP
		if len(payload) >= 28 {
			// op at 6-7, spa at 14-18, tpa at 24-28
			op := uint16(payload[6])<<8 | uint16(payload[7])
			var spa, tpa [4]byte
			copy(spa[:], payload[14:18])
			copy(tpa[:], payload[24:28])
			if op == ARPRequest {
				s.arp.Insert(spa, [6]byte{})
			}
		}
	case 0x0800: // IPv4
		h, data, ok := Parse(payload)
		if !ok {
			return
		}
		s.HandleIPRoute(h.Protocol, h.SrcIP, h.DstIP, data)
	}
}
