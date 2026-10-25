# Teste de ponta a ponta; usa somente dados temporários no D:.
$ErrorActionPreference = 'Stop'
$env:TEMP='D:\tmp'
$env:TMP='D:\tmp'
$testRoot=Join-Path 'D:\tmp' ('machiavelli-api-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot | Out-Null
$server=Join-Path 'D:\cargo-target\debug' 'machiavelli-server.exe'
$process=$null
$previousToken=$env:MACHIAVELLI_TOKEN
$env:MACHIAVELLI_TOKEN=[Guid]::NewGuid().ToString('N')+[Guid]::NewGuid().ToString('N')
$PSDefaultParameterValues['Invoke-RestMethod:Headers']=@{Authorization="Bearer "+$env:MACHIAVELLI_TOKEN}
try {
    $process=Start-Process -FilePath $server -ArgumentList @($testRoot,'0') -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $testRoot 'server.log') -RedirectStandardOutput (Join-Path $testRoot 'stdout.log')
    $timer=[Diagnostics.Stopwatch]::StartNew()
    $base=$null
    while($timer.Elapsed.TotalSeconds -lt 15){
        if($process.HasExited){throw 'server exited during startup'}
        $log=Get-Content -LiteralPath (Join-Path $testRoot 'server.log') -Raw -ErrorAction SilentlyContinue
        if($log -match '127\.0\.0\.1:(\d+)'){$base='http://127.0.0.1:'+$Matches[1];break}
        Start-Sleep -Milliseconds 100
    }
    if(-not $base){throw 'startup timeout'}
    $health=Invoke-RestMethod "$base/health"
    if($health.sequence -ne 0){throw 'nonempty initial store'}
    $id=[Guid]::NewGuid().ToString()
    $node=@{
        meta=@{id=$id;virtu=0.5;fortuna_exposure=0.1;strategic_mass=1.0;hamiltonian_energy=0.0;nash_reach_prob=0.0;cumulative_regret=0.0;vcg_credit_balance=0.0;posture='Expanding';fog_level=0;alliance_cluster=0;dike_count=0;parent_nietzsche_id=[Guid]::Empty.ToString();last_heraclitus_lsn=0;created_at_hlc=1;betrayal_horizon_secs=3600}
        phase=@{q=@(0.3);p=@(0.7);c=@()}
    }
    $body=$node|ConvertTo-Json -Depth 10 -Compress
    $write=Invoke-RestMethod "$base/nodes" -Method Post -ContentType 'application/json' -Body $body
    if($write.sequence -ne 1){throw 'wrong commit sequence'}
    $read=Invoke-RestMethod "$base/nodes/$id"
    if($read.phase.q[0] -ne 0.3){throw 'state differs'}

    try {
        Invoke-RestMethod "$base/nodes" -Headers @{} | Out-Null
        throw 'anonymous access accepted'
    } catch {
        if([int]$_.Exception.Response.StatusCode -ne 401){throw}
    }
    $plan=Invoke-RestMethod "$base/pql" -Method Post -ContentType 'application/json' -Body '{"query":"PRINCE KUHN ITERATIONS 20000 DEADLINE 1000"}'
    if($plan.exploitability -ge 0.001){throw 'Kuhn gap too large'}
    $rollout=Invoke-RestMethod "$base/pql" -Method Post -ContentType 'application/json' -Body (@{query="WARGAME $id STEPS 1000 DT 0.001 DEADLINE 1000"}|ConvertTo-Json)
    if($rollout.persisted -ne $false -or $rollout.steps -ne 1000){throw 'PQL simulation failed'}
    if((Invoke-RestMethod "$base/nodes/$id").phase.q[0] -ne 0.3){throw 'simulation mutated live state'}
    try {
        Invoke-RestMethod "$base/pql" -Method Post -ContentType 'application/json' -Body (@{query="BETRAY $id"}|ConvertTo-Json)|Out-Null
        throw 'unaudited tactical mutation accepted'
    } catch {
        if([int]$_.Exception.Response.StatusCode -ne 409){throw}
    }

    $mcts=Invoke-RestMethod "$base/pql" -Method Post -ContentType 'application/json' -Body (@{query="PRINCE MCTS $id ITERATIONS 512 HORIZON 4 SEED 42 DEADLINE 1000"}|ConvertTo-Json)
    if($mcts.persisted -ne $false -or $mcts.source_sequence -ne 1 -or $mcts.search.iterations -ne 512){throw 'MCTS planning failed'}
    if($mcts.search.selected_action -notin @('hold','damp','retreat')){throw 'MCTS returned an illegal recommendation'}
    if((Invoke-RestMethod "$base/nodes/$id").phase.q[0] -ne 0.3 -or (Invoke-RestMethod "$base/health").sequence -ne 1){throw 'MCTS mutated live state'}
    try {
        Invoke-RestMethod "$base/pql" -Method Post -ContentType 'application/json' -Body (@{query="PRINCE MCTS $([Guid]::NewGuid()) ITERATIONS 1 HORIZON 1 SEED 0"}|ConvertTo-Json)|Out-Null
        throw 'unknown MCTS target accepted'
    } catch { if([int]$_.Exception.Response.StatusCode -ne 404){throw} }
    $node.phase.q=@()
    try {
        Invoke-RestMethod "$base/nodes" -Method Post -ContentType 'application/json' -Body ($node|ConvertTo-Json -Depth 10 -Compress)|Out-Null
        throw 'invalid phase accepted'
    } catch {
        if([int]$_.Exception.Response.StatusCode -ne 422){throw}
    }
    if((Invoke-RestMethod "$base/health").sequence -ne 1){throw 'failed write was committed'}
    # Transação HTTP com escrita em lote e uma única sequência durável.
    $otherId=[Guid]::NewGuid().ToString()
    $node.meta.id=$otherId; $node.phase.q=@(0.4)
    $batch=@{expected_sequence=1;operations=@(@{operation='put';value=$node})}|ConvertTo-Json -Depth 20 -Compress
    $committed=Invoke-RestMethod "$base/transactions" -Method Post -ContentType 'application/json' -Body $batch
    if($committed.sequence -ne 2){throw 'batch sequence differs'}
    try {
        Invoke-RestMethod "$base/transactions" -Method Post -ContentType 'application/json' -Body $batch|Out-Null
        throw 'stale transaction accepted'
    } catch { if([int]$_.Exception.Response.StatusCode -ne 409){throw} }
    $strategic=Invoke-RestMethod "$base/strategic?at_sequence=2"
    if($strategic.sequence -ne 2 -or $strategic.records.Count -ne 0){throw 'strategic snapshot query failed'}
    $adjacency=Invoke-RestMethod "$base/nodes/$otherId/adjacency"
    if($adjacency.sequence -ne 2 -or $adjacency.adjacency.Count -ne 0){throw 'adjacency query failed'}
    # Arrêt abrupt de notre propre processus de test, pour vérifier la reprise.
    Stop-Process -Id $process.Id
    $process.WaitForExit()
    $process=$null

    $reopened=& 'D:\cargo-target\debug\machiavelli.exe' $testRoot get $id
    if($LASTEXITCODE -ne 0){throw 'reopen after process exit failed'}
    if(($reopened|ConvertFrom-Json).meta.id -ne $id){throw 'record lost after restart'}
    & 'D:\cargo-target\debug\machiavelli.exe' $testRoot delete $id | Out-Null
    if($LASTEXITCODE -ne 0){throw 'delete failed'}
    $stats=& 'D:\cargo-target\debug\machiavelli.exe' $testRoot stats | ConvertFrom-Json
    if($stats.nodes -ne 1 -or $stats.sequence -ne 3){throw 'delete not durable'}
    Write-Output "PASS: authenticated REST, PQL/CFR/MCTS, atomic batches, stale-write rejection, process-exit recovery and CLI delete. Test data: $testRoot"
} finally {
    $env:MACHIAVELLI_TOKEN=$previousToken
    $PSDefaultParameterValues.Remove('Invoke-RestMethod:Headers')
    if($process -and -not $process.HasExited){Stop-Process -Id $process.Id; $process.WaitForExit()}
}
