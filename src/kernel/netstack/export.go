package netstack

// export.go — TinyGo 导出层（方案A：TinyGo 编译 Go 协议栈为独立 wasm 模块，
// 内核内嵌 wasm 解释器后通过 //export 函数调用这些协议逻辑）。
// 这些函数用 //export 导出为 wasm 符号，供宿主（内核 wasm runtime）调用。

// NetChecksum 计算一补码 Internet 校验和。
//
//export NetChecksum
func NetChecksum(data []byte) uint16 {
	return checksum(data)
}

// NetUDPChecksum 计算 UDP 伪头校验和。
//
//export NetUDPChecksum
func NetUDPChecksum(src, dst [4]byte, uh, payload []byte) uint16 {
	return UDPChecksum(src, dst, uh, payload)
}

// NetHandleEthernetFrame 处理一帧以太网帧（协议栈分发入口）。
// 宿主把 NIC 收到的帧喂进来，协议栈内部路由到对应 socket。
//
//export NetHandleEthernetFrame
func NetHandleEthernetFrame(s *Stack, frame []byte) {
	s.HandleEthernetFrame(frame)
}

// NetActiveOpen / NetPassiveOpen / NetHandleSegment：TCP 状态机入口。
//
//export NetActiveOpen
func NetActiveOpen(s *Socket) { s.ActiveOpen() }

//export NetPassiveOpen
func NetPassiveOpen(s *Socket) { s.PassiveOpen() }

//export NetHandleSegment
func NetHandleSegment(s *Socket, flags byte) { s.HandleSegment(flags) }

// NetClose 应用关闭。
//
//export NetClose
func NetClose(s *Socket) { s.Close() }
