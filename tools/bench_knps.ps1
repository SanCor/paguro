param(
    [Parameter(Mandatory = $true)]
    [string]$Engine,
    [int]$Repetitions = 5
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$positions = @(
    @{ Name = "startpos"; Command = "position startpos"; Depth = 11 },
    @{ Name = "kiwipete"; Command = "position fen r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"; Depth = 10 },
    @{ Name = "endgame"; Command = "position fen 8/7p/5k2/5p2/p1p2P2/Pr1pPK2/1P1R3P/8 b - - 0 1"; Depth = 16 }
)

function Invoke-Search($position) {
    $psi = [Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = (Resolve-Path $Engine)
    $psi.WorkingDirectory = $root
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $process = [Diagnostics.Process]::Start($psi)

    $process.StandardInput.WriteLine("uci")
    $process.StandardInput.WriteLine("setoption name OwnBook value false")
    $process.StandardInput.WriteLine("isready")
    $process.StandardInput.WriteLine($position.Command)
    $process.StandardInput.WriteLine("go depth $($position.Depth)")
    $process.StandardInput.Flush()

    $info = $null
    $best = $null
    while (-not $process.StandardOutput.EndOfStream) {
        $line = $process.StandardOutput.ReadLine()
        if ($line -match "^info depth $($position.Depth) " -and $line -notmatch "bound") {
            $info = $line
        }
        if ($line -match "^bestmove ") {
            $best = ($line -split "\s+")[1]
            break
        }
    }
    $process.StandardInput.WriteLine("quit")
    if (-not $process.WaitForExit(5000)) {
        $process.Kill()
    }
    if (-not $info -or -not $best) {
        throw "Incomplete search for $($position.Name)"
    }

    $nodes = [uint64]([regex]::Match($info, " nodes (\d+)").Groups[1].Value)
    $time = [uint64]([regex]::Match($info, " time (\d+)").Groups[1].Value)
    $score = [regex]::Match($info, " score (cp|mate) (-?\d+)").Groups[0].Value.Trim()
    [pscustomobject]@{
        Name = $position.Name
        Nodes = $nodes
        TimeMs = $time
        Knps = [math]::Round($nodes / [math]::Max($time, 1), 1)
        Score = $score
        BestMove = $best
    }
}

foreach ($position in $positions) {
    $runs = for ($i = 0; $i -lt $Repetitions; $i++) {
        Invoke-Search $position
    }
    $reference = $runs[0]
    foreach ($run in $runs) {
        if ($run.Nodes -ne $reference.Nodes -or
            $run.Score -ne $reference.Score -or
            $run.BestMove -ne $reference.BestMove) {
            throw "Non-deterministic result for $($position.Name)"
        }
    }
    $sorted = @($runs.Knps | Sort-Object)
    $median = $sorted[[math]::Floor($sorted.Count / 2)]
    [pscustomobject]@{
        Position = $position.Name
        Depth = $position.Depth
        Nodes = $reference.Nodes
        MedianKnps = $median
        MinKnps = $sorted[0]
        MaxKnps = $sorted[-1]
        Score = $reference.Score
        BestMove = $reference.BestMove
    }
}
