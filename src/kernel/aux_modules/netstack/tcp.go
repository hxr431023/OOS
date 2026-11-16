package netstack

// TCP control flags (RFC 793 header byte 13).
const (
	FlagFIN = 0x01
	FlagSYN = 0x02
	FlagRST = 0x04
	FlagPSH = 0x08
	FlagACK = 0x10
)

// ActiveOpen initiates a client connect: Closed -> SynSent.
func (s *Socket) ActiveOpen() {
	if s.Type != TCP || s.State != Closed {
		return
	}
	s.State = SynSent
}

// PassiveOpen enters listen: Closed -> Listen.
func (s *Socket) PassiveOpen() {
	if s.Type != TCP || s.State != Closed {
		return
	}
	s.State = Listen
}

// HandleSegment applies a TCP segment to the state machine and returns
// whether the state changed. Flags is the byte-13 control bits.
func (s *Socket) HandleSegment(flags byte) {
	switch s.State {
	case Listen:
		// peer SYN -> server responds SYN+ACK -> SynReceived
		if flags&FlagSYN != 0 {
			s.State = SynReceived
		}
	case SynSent:
		// SYN + ACK -> Established
		if flags&FlagSYN != 0 && flags&FlagACK != 0 {
			s.State = Established
		} else if flags&FlagRST != 0 {
			s.State = Closed
		}
	case SynReceived:
		if flags&FlagACK != 0 {
			s.State = Established
		}
	case Established:
		if flags&FlagFIN != 0 {
			s.State = CloseWait
		}
	case FinWait1:
		if flags&FlagFIN != 0 && flags&FlagACK != 0 {
			s.State = TimeWait
		} else if flags&FlagACK != 0 {
			s.State = FinWait2
		}
	case FinWait2:
		if flags&FlagFIN != 0 {
			s.State = TimeWait
		}
	case CloseWait:
		// app closes -> LastAck (then server sends FIN)
	case LastAck:
		if flags&FlagACK != 0 {
			s.State = Closed
		}
	case Closing:
		if flags&FlagACK != 0 {
			s.State = TimeWait
		}
	}
}

// Close initiates active close from Established: -> FinWait1.
func (s *Socket) Close() {
	if s.State == Established {
		s.State = FinWait1
	} else if s.State == CloseWait {
		s.State = LastAck
	}
}
