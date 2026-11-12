import socket
def cmd(c):
    try:
        s=socket.create_connection(('127.0.0.1',9000),timeout=3)
    except Exception as e:
        return f"[connect fail: {e}]"
    s.settimeout(2)
    try:
        s.sendall((c+'\n').encode())
    except Exception as e:
        s.close(); return f"[send fail: {e}]"
    data=b''
    try:
        while True:
            chunk=s.recv(4096)
            if not chunk: break
            data+=chunk
    except: pass
    s.close()
    return data.decode(errors='replace')
for c in ['version','ls /','ls /system/proc','cat /system/proc/version','cat /system/proc/meminfo','cat /system/proc/cpuinfo','ps']:
    print('=== '+c+' ===')
    print(cmd(c))
    print()
