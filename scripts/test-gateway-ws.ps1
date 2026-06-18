#Requires -Version 5.1
[CmdletBinding()]
param()

$client = New-Object System.Net.WebSockets.ClientWebSocket
$cts = New-Object System.Threading.CancellationTokenSource
$uri = [System.Uri]'ws://127.0.0.1:18800/ws'

$connect = $client.ConnectAsync($uri, $cts.Token)
$connect.Wait()
Write-Host 'WebSocket connected'

$buffer = New-Object byte[] 4096
$seg = New-Object System.ArraySegment[byte](, $buffer)
$recv = $client.ReceiveAsync($seg, $cts.Token)
$recv.Wait()
$text = [System.Text.Encoding]::UTF8.GetString($buffer, 0, $recv.Result.Count)
Write-Host "Received: $text"

$client.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, 'test', $cts.Token).Wait()
