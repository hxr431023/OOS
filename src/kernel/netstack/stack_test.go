package netstack

import "testing"

func TestTCPHandshake(t *testing.T) {
	s := NewSocket(1, TCP, 100)
	s.PassiveOpen()
	if s.State != Listen {
		t.Fatalf("expected Listen, got %v", s.State)
	}
	// client SYN -> server Listen -> SynReceived; server SYN+ACK; client ACK -> Established
	s.HandleSegment(FlagSYN)
	if s.State != SynReceived {
		t.Fatalf("expected SynReceived, got %v", s.State)
	}
	s.HandleSegment(FlagACK)
	if s.State != Established {
		t.Fatalf("expected Established, got %v", s.State)
	}
	// peer FIN -> CloseWait
	s.HandleSegment(FlagFIN)
	if s.State != CloseWait {
		t.Fatalf("expected CloseWait, got %v", s.State)
	}
	s.Close() // CloseWait -> LastAck
	if s.State != LastAck {
		t.Fatalf("expected LastAck, got %v", s.State)
	}
	s.HandleSegment(FlagACK)
	if s.State != Closed {
		t.Fatalf("expected Closed, got %v", s.State)
	}
}

func TestIPChecksum(t *testing.T) {
	// well-known IPv4 header checksum self-check: all-zero header checksum is 0xFFFF
	h := IPv4Header{SrcIP: [4]byte{192, 168, 1, 1}, DstIP: [4]byte{10, 0, 0, 1}}
	pkt := h.Encode(nil)
	// recompute should be 0
	if checksum(pkt[:20]) != 0 {
		t.Fatalf("IP header checksum invalid: %04x", checksum(pkt[:20]))
	}
}

func TestUDPSocketRoute(t *testing.T) {
	st := NewStack()
	st.LocalIP = [4]byte{192, 168, 1, 1}
	sock := st.CreateSocket(UDP, 100)
	sock.LocalPort = 8888
	// incoming UDP from 10.0.0.2:1234 to 192.168.1.1:8888
	udp := UDPEncode(&UDPHeader{SrcPort: 1234, DstPort: 8888, Length: 8 + 5}, []byte("hello"))
	ip := &IPv4Header{Protocol: ProtoUDP, SrcIP: [4]byte{10, 0, 0, 2}, DstIP: [4]byte{192, 168, 1, 1}}
	datagram := ip.Encode(udp)
	h, data, ok := Parse(datagram)
	if !ok {
		t.Fatal("parse failed")
	}
	st.HandleIPRoute(h.Protocol, h.SrcIP, h.DstIP, data)
	if string(sock.RxBuf) != "hello" {
		t.Fatalf("expected hello in rx, got %q", sock.RxBuf)
	}
}
