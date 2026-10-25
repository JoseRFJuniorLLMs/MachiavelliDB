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

## Verificação completa em um comando

~~~powershell
Set-Location D:\DEV\MachiavelliDB
.\scripts\verify.ps1 -RocksDB -Lean
~~~

Executa formatação, suíte completa, Clippy com warnings negados, build, smoke HTTP,
RocksDB no WSL e provas Lean. Os logs e results.json ficam em uma pasta identificada
em D:\tmp. As opções -RocksDB e -Lean podem ser omitidas quando esses ambientes
não estiverem instalados. -Qualification executa também os benchmarks de referência.

## Treino persistente de Kuhn

~~~powershell
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-training train-kuhn 20000 1000
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-training train-kuhn 20000 1000
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-training stats
~~~

A segunda chamada retoma os regrets e as médias salvos pela primeira. A saída
informa iterations desta chamada e cumulative_iterations; o orçamento pode
interromper antes do limite. O checkpoint é salvo com uma única transação.
Backup/restauração inclui esse estado. PRINCE KUHN na API continua sendo uma
consulta independente, sem gravar o treinamento.

## Transações e registros estratégicos

A CLI aceita commit ARQUIVO_JSON [SEQUÊNCIA_ESPERADA] e records para inspeção.
O arquivo contém uma lista de operações; o formato completo está em STORAGE.md.

REST autenticado:
- POST /transactions: {"expected_sequence":0,"operations":[...]}.
- GET /strategic?family=regrets&limit=100&offset=0&at_sequence=SEQUÊNCIA.
- GET /nodes/{uuid}/adjacency: entradas de saída e entrada com a sequência lida.

O servidor confirma todas as operações do lote ou nenhuma. Sequência divergente
retorna 409. A API recusa posturas táticas, escrita de arestas/diques e exclusão
desses registros sem a integração administrativa real. A CLI é uma ferramenta
local de operador, com acesso direto à pasta do banco; não é uma fronteira de roles.

## Planejamento MCTS

~~~powershell
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-demo plan 6a54d3ce-57db-476e-bc63-4d26baf20450 2000 8 42 1000
~~~

Ou POST /pql com:

~~~json
{"query":"PRINCE MCTS 6a54d3ce-57db-476e-bc63-4d26baf20450 ITERATIONS 2000 HORIZON 8 SEED 42 DEADLINE 1000"}
~~~

O nó precisa existir. A resposta informa visitas, médias e ação recomendada no
modelo de referência; persisted é false. Nenhuma ação tática é executada.
Consulte PLANNING.md para os limites do modelo e o filtro de partículas.

## Operação Linux/WSL (ambiente de execução)

O Windows fica só para edição. Compilação, testes e servidor correm no Ubuntu-24.04
do WSL (disco em D:\WSL); dados e temporários em /mnt/d/tmp ou no disco do WSL.

~~~bash
wsl -d Ubuntu-24.04 -e bash /mnt/d/DEV/MachiavelliDB/scripts/verify.sh
~~~

Gates: formatação, testes com rocksdb-backend, Clippy -D warnings, build release e
smoke REST/PQL/MCTS com SIGKILL e reabertura pela CLI. --qualification acrescenta
os benchmarks CFR e arena. Para incluir o teste ponta a ponta com o Heraclitus:

~~~bash
wsl -d Ubuntu-24.04 -e bash -lc 'MACHIAVELLI_E2E_HERACLITUS=$HOME/cargo-target/heraclitus/debug/heraclitus-server bash /mnt/d/DEV/MachiavelliDB/scripts/verify.sh'
~~~

O binário vem de cargo build -p heraclitus-server no HeraclitusDB com o protocolo
external-intent-v1. O script arranca uma instância própria (pasta temporária,
portas aleatórias em 127.0.0.1, token gerado) e nunca o serviço heraclitus do
operador. Binários: $HOME/cargo-target/machiavelli/release/machiavelli-server e
machiavelli. CUDA: scripts/cuda.sh (sai com 77 se não houver nvcc/GPU no WSL).

Ao arrancar o servidor no Linux, aponte TMPDIR para o disco ext4 do WSL (por
exemplo $HOME/tmp; o disco virtual fica em D:\WSL). A arena e os rollouts do
Centauro usam ficheiros temporários mapeados em memória; em /mnt/d (drvfs) o fork
da arena mediu 152 µs de mediana contra 2,2 µs no ext4.

## Ações táticas autorizadas pelo Heraclitus

POST /actions só existe quando o servidor arranca com:
- HERACLITUS_ENDPOINT: http://127.0.0.1:PORTA (gRPC do Heraclitus, só loopback).
- HERACLITUS_TOKEN: credencial do Heraclitus, 32+ caracteres ASCII. As operações
  external-*-v1 exigem hoje o papel Admin; ainda não há um papel dedicado.
- MACHIAVELLI_QUORUM_FILE (opcional): {"keys":["hex32",...],"faults":f,"epoch":e}.
  Sem ele, BETRAY, FORTIFY e FIRE_DIKE são recusados.

~~~json
{"operation_id":"ally-2026-10-25-1","expected_sequence":1,"expires_ms":1792940000000,
 "action":{"action":"ally","from":"UUID-A","to":"UUID-B"},"votes":[]}
~~~

operation_id: 1..128 caracteres [A-Za-z0-9-]. expires_ms: epoch em ms, entre 1 ms e
5 min no futuro. Ações: observe, ally, tribute (créditos inteiros), betray (nó
Expendable sem coordenadas constitucionais), fortify, dike (ttl_ms até 24 h) e
fire_dike. Votos: assinaturas Ed25519 de signing_message(digest, expected_sequence,
epoch, expires_ms), com o digest de Request::signing_digest.

Fluxo: o pedido fica pendente no banco (sequência +1); o Heraclitus grava a
autorização; efeito e marcador de resultado entram num único commit (+2); o
resultado é confirmado no Heraclitus (+3). Enquanto houver uma operação pendente sem
resultado, as outras escritas recebem 409. Se o Heraclitus estiver em baixo, a
resposta devolve a entrada pendente e a recuperação tenta de novo a cada 2 s, também
após reinício. Expirada antes do commit local, a operação fecha sem efeito.
Repetir o mesmo pedido devolve a mesma entrada; o mesmo ID com outro conteúdo é 409.
GET /actions/{operation_id} mostra recibo, resultado e acknowledged.
