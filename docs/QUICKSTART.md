# Operação local

~~~powershell
Set-Location D:\DEV\MachiavelliDB
.\scripts\cargo.ps1 build --workspace --offline
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-demo init
$env:MACHIAVELLI_TOKEN=[Guid]::NewGuid().ToString("N")+[Guid]::NewGuid().ToString("N")
D:\cargo-target\debug\machiavelli-server.exe D:\tmp\machiavelli-demo 8787
~~~

O servidor fica em primeiro plano. Use Ctrl+C para encerrar e liberar o lock antes
de abrir a mesma pasta pela CLI. Instâncias para pastas diferentes são independentes.

REST local:
- GET /health: modo, contagem e sequência.
- GET /nodes: lista de nós.
- GET /nodes/{uuid}: um nó.
- POST /nodes: JSON de ContinentiaNode para criar/substituir.
- DELETE /nodes/{uuid}: remoção sujeita às invariantes.

Exemplo de JSON para POST /nodes ou arquivo passado à CLI put:

~~~json
{
  "meta": {
    "id": "6a54d3ce-57db-476e-bc63-4d26baf20450",
    "virtu": 0.5, "fortuna_exposure": 0.1, "strategic_mass": 1.0,
    "hamiltonian_energy": 0.0, "nash_reach_prob": 0.0,
    "cumulative_regret": 0.0, "vcg_credit_balance": 0.0,
    "posture": "Expanding", "fog_level": 0, "alliance_cluster": 0,
    "dike_count": 0, "parent_nietzsche_id": "00000000-0000-0000-0000-000000000000",
    "last_heraclitus_lsn": 0, "created_at_hlc": 1,
    "betrayal_horizon_secs": 3600
  },
  "phase": {"q": [0.3], "p": [0.7], "c": []}
}
~~~

CLI (servidor encerrado para a mesma pasta):

~~~powershell
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-demo stats
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-demo put D:\tmp\node.json
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-demo list
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-demo get 6a54d3ce-57db-476e-bc63-4d26baf20450
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-demo simulate 6a54d3ce-57db-476e-bc63-4d26baf20450 10000 0.001
~~~

simulate usa oscilador harmônico de referência, massa por postura e estado copiado;
não persiste o resultado. O modelo físico é informado na resposta JSON.

Solver: crie D:\tmp\game.json com a matriz dos dois jogadores:
~~~json
{"rows":2,"cols":2,"row_payoff":[3,0,0,1],"col_payoff":[-3,0,0,-1]}
~~~
~~~powershell
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-demo solve D:\tmp\game.json 50000 1000
~~~
O comando retorna probabilidades, valor, limites de melhores respostas e
explorabilidade medida; não altera o banco nem promete gap abaixo de um limiar
quando o prazo expira. Somente matrizes de soma zero são aceitas.

Consulte STATUS.md para limitações da alpha e trabalho ainda pendente.

## Autenticação, PQL, gRPC e observatório

REST exige Authorization: Bearer TOKEN, salvo /health e /observatory.
GET /nodes aceita limit=1..1000, after=UUID e at_sequence=sequência. Se o banco
mudar entre páginas, at_sequence retorna conflito para reiniciar a paginação.

POST /pql recebe {"query":"SCOUT ALL"} ou
{"query":"PRINCE KUHN ITERATIONS 20000 DEADLINE 1000"}.
WARGAME UUID STEPS 10000 DT 0.001 DEADLINE 1000 simula sem persistir.
Os verbos de mutação tática permanecem bloqueados, sem fabricar autorização.

O gRPC machiavelli.v1.Machiavelli compartilha porta e token com REST.
Contrato: crates/machiavelli-proto/proto/machiavelli.proto.
Put/Get usam os codecs binários de metadados/fase, não JSON de floats.

Lo Stato fica em http://127.0.0.1:8787/observatory. Informe o token no formulário;
ele fica somente na memória da página, sem localStorage.

Backup e restauração (destinos novos; servidor encerrado para a pasta de origem):
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-demo backup D:\tmp\machiavelli-backup
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-restored restore D:\tmp\machiavelli-backup

RocksDB nativo: scripts/rocks.ps1 repete os testes de storage no WSL.
Para compilar o servidor com esse backend:
scripts/rocks.ps1 build -p machiavelli-api --features machiavelli-storage/rocksdb-backend
O binário Linux fica em /home/junior/cargo-target/machiavelli/debug/machiavelli-server.
Os dados devem ficar em uma pasta nova no D: ou no disco WSL armazenado no D:.
