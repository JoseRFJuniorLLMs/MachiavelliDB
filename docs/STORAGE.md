# Persistência estratégica e transações

O backend WAL e o backend opcional RocksDB oferecem a mesma interface Store.
Cada commit valida todo o lote, sincroniza a gravação durável e só então publica
o novo snapshot. Uma falha de validação mantém o conteúdo e a sequência anteriores.
commit_at exige a sequência esperada sob o mesmo writer exclusivo.

## Registros materializados

| Família | Tipo | RocksDB |
|---|---|---|
| Nós | metadados binários + PhaseVector | cf_continentia / cf_phase_space |
| edges | StrategicEdge | cf_adj_strategic |
| payoffs | PayoffMatrix vinculada à aresta | cf_payoff_tensors |
| infosets | jogador e ações ordenadas | cf_infosets |
| regrets | regrets, somas de estratégia e iterações | cf_regret_tables |
| dikes | CompiledDike com TTL e topologia originais | cf_dikes |
| ledger | resultado de leilão VCG | cf_vcg_ledger |

As outras quatro CFs continuam reservadas. Gravar um plano não executa suas ações;
um registro de leilão não transfere créditos, RAM ou CPU. O resultado do leilão é
recalculado antes de aceitar o registro, que não pode ser substituído ou apagado.
Os identificadores de bidders são históricos e não impedem a remoção posterior de nós.

Arestas exigem endpoints existentes, distintos e mutáveis, peso finito e UUID não
nulo. Alterações de arestas e payoffs associados a nós Fortified/Betrayed são
recusadas. O índice bidirecional acompanha criação, retargeting e remoção, inclusive
quando a mesma aresta muda várias vezes em um lote. RocksDB verifica o índice ao
reabrir, recusando inconsistências em vez de repará-las silenciosamente.

Remova payoffs antes de remover a aresta; remova arestas/diques que referenciem um nó
antes de apagá-lo. Remova regrets antes de alterar a definição ou apagar seu infoset.
A ordem das operações dentro do lote importa. Todos os passos permanecem atômicos.

## Operação pela CLI

~~~powershell
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-demo records
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-demo commit D:\tmp\batch.json 12
~~~

O segundo comando exige que a sequência atual seja 12. O arquivo JSON contém
operações com operation e value:

| operation | value |
|---|---|
| put | ContinentiaNode, conforme QUICKSTART.md |
| delete | UUID textual do nó |
| put_strategic | StrategicRecord tipado |
| delete_strategic | {"family":"edges","id":[16 bytes do UUID]} |

StrategicRecord tem o discriminador type: edge, payoff, infoset, regrets, dike ou
auction. UUIDs são strings; InfosetHash/TriggerHash são arrays de 32 bytes. O Rust
expõe RecordKey::new para construir as chaves sem conversões manuais.

Exemplo de criação de aresta entre dois nós existentes por operador local:

~~~powershell
$nodeA='6a54d3ce-57db-476e-bc63-4d26baf20450'
$nodeB='7a54d3ce-57db-476e-bc63-4d26baf20450'
$operations=@(@{operation='put_strategic';value=@{type='edge';edge=@{
    id=[Guid]::NewGuid().ToString();from=$nodeA;to=$nodeB;kind='Threat';weight=0.5
}}})
ConvertTo-Json -InputObject $operations -Depth 10 | Set-Content D:\tmp\batch.json
~~~

A pasta do banco deve estar liberada pelo servidor para a CLI operar.
A CLI é uma ferramenta administrativa local; a API autenticada continua bloqueando
mutações táticas até a implementação do contrato externo de autorização.

## Snapshots, formatos e recuperação

Os mapas AVL são persistentes: snapshots compartilham raízes e payloads imutáveis;
uma atualização copia apenas o caminho da árvore, sem duplicar todos os vetores.
Isso não implica que a arena mmap esteja integrada à persistência transacional.

O envelope WAL v2 aceita os novos registros e lê envelopes v1. O frame MCV1 e o
limite de 16 MiB permanecem. Não faça downgrade para leitores antigos após gravar
no formato novo. As operações de nós continuam usando os codecs binários exatos;
os registros estratégicos usam JSON com parsing float_roundtrip.

Backup v2 inclui todos os registros e mantém a leitura de backups v1. A restauração
constrói uma pasta nova sob lock, restaura primeiro a topologia e por último as
posturas finais; só publica o marcador RESTORED ao terminar. Falhas deixam a pasta
incompleta para inspeção. Não reutilize um destino incompleto como um banco pronto.
O histórico de transações e a sequência local original não são restaurados.
Diques conservam seu TTL/topologia original e precisam de nova qualificação antes
de disparar em uma instância restaurada.

## Solver retomável

Game::resume valida digest do jogo, jogador, ações, dimensões, finitude e não
negatividade dos valores. O contador absoluto preserva os pesos lineares do CFR+.
train-kuhn guarda definições e tabelas em uma única transação; checkpoints parciais
ou com contadores divergentes são recusados. O teste compara exatamente 500+500
iterações com 1.000 contínuas, além de testar reinício e backup entre processos.
