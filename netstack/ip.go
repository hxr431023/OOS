package netstack

// IPv4 datagram header layout (RFC 791), 20 bytes + options.
// The stack processes/encapsulates IPv4 packets for the NIC IO-stream objects.
type IPv4Header struct {
	VersionIHL  uint8  // version(4) + IHL(4)
	ToS         uint8
	TotalLen    uint16
	ID          uint16
	FlagsFrag   uint16
	TTL         uint8
	Protocol    uint8
	Checksum    uint16
	SrcIP       [4]byte
	DstIP       [4]byte
}

// Protocol numbers.
const (
	ProtoICMP = 1
	ProtoTCP  = 6
	ProtoUDP  = 17
)

// Encode serializes the header + payload into a full IPv4 datagram.
func (h *IPv4Header) Encode(payload []byte) []byte {
	pkt := make([]byte, 20+len(payload))
	pkt[0] = 0x45 // version 4, IHL 5
	pkt[1] = h.ToS
	pkt[2] = byte(h.TotalLen >> 8)
	pkt[3] = byte(h.TotalLen)
	pkt[4] = byte(h.ID >> 8)
	pkt[5] = byte(h.ID)
	pkt[6] = byte(h.FlagsFrag >> 8)
	pkt[7] = byte(h.FlagsFrag)
	pkt[8] = h.TTL
	pkt[9] = h.Protocol
	copy(pkt[12:16], h.SrcIP[:])
	copy(pkt[16:20], h.DstIP[:])
	// checksum over header only
	h.Checksum = checksum(pkt[:20])
	pkt[10] = byte(h.Checksum >> 8)
	pkt[11] = byte(h.Checksum)
	copy(pkt[20:], payload)
	return pkt
}

// Parse decodes a raw IPv4 datagram into header + payload.
func Parse(pkt []byte) (*IPv4Header, []byte, bool) {
	if len(pkt) < 20 {
		return nil, nil, false
	}
	h := &IPv4Header{
		VersionIHL: pkt[0],
		ToS:        pkt[1],
		TotalLen:   uint16(pkt[2])<<8 | uint16(pkt[3]),
		ID:         uint16(pkt[4])<<8 | uint16(pkt[5]),
		FlagsFrag:  uint16(pkt[6])<<8 | uint16(pkt[7]),
		TTL:        pkt[8],
		Protocol:   pkt[9],
		Checksum:   uint16(pkt[10])<<8 | uint16(pkt[11]),
	}
	copy(h.SrcIP[:], pkt[12:16])
	copy(h.DstIP[:], pkt[16:20])
	ihl := int(pkt[0]&0x0F) * 4
	if ihl < 20 || len(pkt) < ihl {
		return nil, nil, false
	}
	return h, pkt[ihl:], true
}

// checksum computes the standard one's-complement Internet checksum.
func checksum(data []byte) uint16 {
	var sum uint32
	for i := 0; i+1 < len(data); i += 2 {
		sum += uint32(data[i])<<8 | uint32(data[i+1])
	}
	if len(data)%2 == 1 {
		sum += uint32(data[len(data)-1]) << 8
	}
	for sum>>16 != 0 {
		sum = (sum & 0xFFFF) + (sum >> 16)
	}
	return ^uint16(sum)
}
