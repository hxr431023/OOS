package netstack

// ARP (RFC 826) table and packet handling for NIC IO-stream objects.
// Pure logic, no stdlib deps (kernel-mode TinyGo-friendly).

// ARPEntry maps an IPv4 to a MAC.
type ARPEntry struct {
	IP  [4]byte
	MAC [6]byte
}

// ARPTable is a small fixed-capacity table (kernel: no GC, use slice not map).
type ARPTable struct {
	entries []ARPEntry
}

// NewARPTable returns an empty table.
func NewARPTable() *ARPTable { return &ARPTable{} }

// Lookup finds the MAC for an IP.
func (t *ARPTable) Lookup(ip [4]byte) ([6]byte, bool) {
	for i := range t.entries {
		if t.entries[i].IP == ip {
			return t.entries[i].MAC, true
		}
	}
	return [6]byte{}, false
}

// Insert adds/updates an IP->MAC entry.
func (t *ARPTable) Insert(ip [4]byte, mac [6]byte) {
	for i := range t.entries {
		if t.entries[i].IP == ip {
			t.entries[i].MAC = mac
			return
		}
	}
	t.entries = append(t.entries, ARPEntry{IP: ip, MAC: mac})
}

// ARP hardware/protocol types.
const (
	ARPHwEthernet = 1
	ARPProtoIPv4  = 0x0800
	ARPRequest    = 1
	ARPReply      = 2
)

// ARPEncode builds an ARP packet (28 bytes, hw=ethernet, proto=IPv4).
func ARPEncode(op uint16, sha [6]byte, spa [4]byte, tha [6]byte, tpa [4]byte) []byte {
	p := make([]byte, 28)
	p[0] = 0 // htype hi
	p[1] = byte(ARPHwEthernet)
	p[2] = 0x08 // ptype hi
	p[3] = 0x00 // ptype lo (IPv4)
	p[4] = 6    // hlen
	p[5] = 4    // plen
	p[6] = byte(op >> 8)
	p[7] = byte(op)
	copy(p[8:14], sha[:])
	copy(p[14:18], spa[:])
	copy(p[18:24], tha[:])
	copy(p[24:28], tpa[:])
	return p
}
