package netstack

// UDP datagram (RFC 768). Pure logic, no stdlib deps.

// UDPHeader is the 8-byte UDP header.
type UDPHeader struct {
	SrcPort  uint16
	DstPort  uint16
	Length   uint16
	Checksum uint16
}

// UDPEncode serializes the UDP header + payload.
func UDPEncode(h *UDPHeader, payload []byte) []byte {
	total := 8 + len(payload)
	b := make([]byte, total)
	b[0] = byte(h.SrcPort >> 8)
	b[1] = byte(h.SrcPort)
	b[2] = byte(h.DstPort >> 8)
	b[3] = byte(h.DstPort)
	b[4] = byte(total >> 8)
	b[5] = byte(total)
	b[6] = byte(h.Checksum >> 8)
	b[7] = byte(h.Checksum)
	copy(b[8:], payload)
	return b
}

// ParseUDP splits a UDP datagram into header + payload.
func ParseUDP(b []byte) (*UDPHeader, []byte, bool) {
	if len(b) < 8 {
		return nil, nil, false
	}
	h := &UDPHeader{
		SrcPort:  uint16(b[0])<<8 | uint16(b[1]),
		DstPort:  uint16(b[2])<<8 | uint16(b[3]),
		Length:   uint16(b[4])<<8 | uint16(b[5]),
		Checksum: uint16(b[6])<<8 | uint16(b[7]),
	}
	return h, b[8:], true
}

// UDPChecksum computes the UDP pseudo-header checksum over src/dst IP +
// UDP header + payload (RFC 768). src/dst are the IPv4 addresses.
func UDPChecksum(src, dst [4]byte, uh, payload []byte) uint16 {
	// pseudo-header: src(4) + dst(4) + zero(1) + proto(1) + udp_len(2)
	ph := make([]byte, 12)
	copy(ph[0:4], src[:])
	copy(ph[4:8], dst[:])
	ph[8] = 0
	ph[9] = ProtoUDP
	udpLen := len(uh) + len(payload)
	ph[10] = byte(udpLen >> 8)
	ph[11] = byte(udpLen)
	var sum uint32
	comb := func(d []byte) {
		for i := 0; i+1 < len(d); i += 2 {
			sum += uint32(d[i])<<8 | uint32(d[i+1])
		}
		if len(d)%2 == 1 {
			sum += uint32(d[len(d)-1]) << 8
		}
	}
	comb(ph)
	comb(uh)
	comb(payload)
	for sum>>16 != 0 {
		sum = (sum & 0xFFFF) + (sum >> 16)
	}
	c := ^uint16(sum)
	if c == 0 {
		c = 0xFFFF
	}
	return c
}
