# Estado verificável do MachiavelliDB

O workspace contém os 25 crates da arquitetura (incluindo invariants, adicionado
ao mapa original de 24). Isso não significa que todas as 18 SPECs estejam concluídas.
Há uma implementação operacional local e componentes experimentais com limites explícitos.

## Implementado e verificado

- Core, integradores simpléticos, Hamiltoniano, derivadas e mapa tangente.
- WAL próprio, validação transacional de nós, recuperação de cauda incompleta,
  snapshots imutáveis, backup/restauração com checksum e recusa de sobrescrita.
  Arestas, payoffs, infosets/regrets, diques e resultados VCG também são duráveis.
  Transações têm compare-and-commit por sequência e índices de adjacência bidirecionais.
- Backend RocksDB opcional com 14 column families; testes nativos no WSL/D:.
  Onze estão materializadas: nós/fase, arestas/adjacências, payoffs, infosets,
  regrets, diques, ledger, crenças, modelos neurais e o outbox Trinitas.
  Deception, invariants e bft_quorum continuam reservadas; sua presença não
  significa integração desses subsistemas.
- Ações táticas autorizadas (Trinitas, POST /actions): observe, ally, tribute,
  betray, fortify, dike e fire_dike. Cada pedido fica num outbox durável, recebe
  autorização no Heraclitus (external-intent-v1), aplica efeito e marcador de
  resultado num único commit e confirma o resultado no Heraclitus. Uma operação
  autorizada sem desfecho bloqueia as outras escritas; a recuperação retoma-a após
  reinício sem repetir o efeito, e a expiração fecha-a sem efeito. Créditos usam
  unidades inteiras. betray, fortify e fire_dike exigem quórum Ed25519 configurado
  no servidor. Testado com servidor simulado e com um Heraclitus real no WSL.
- Deep CFR de referência em CPU: amostragem externa, reservatórios limitados, MLPs
  de vantagem e de estratégia média, checkpoints que retomam exatamente. CLI
  train-deep grava o modelo em cf_neural_models. O teste cobre retoma exata e
  estratégias válidas no Kuhn; não há medição de convergência nem treino em escala.
- Árvore AVL persistente: snapshots O(1), atualizações por cópia do caminho O(log n),
  compartilhando payloads e subárvores. A arena mmap é um componente separado.
- Arena mmap com páginas CoW em forks, snapshots antigos e promoção por CAS.
- CFR+ extensivo, validação de perfect recall, chance, políticas compartilhadas
  por information set e melhores respostas que respeitam informação oculta.
  Jogos de referência: Kuhn, Leduc e ataque/defesa de duas rotas.
  Checkpoints vinculados ao digest do jogo preservam regrets e médias lineares;
  retomar 500+500 iterações reproduz exatamente 1.000 contínuas no teste de Kuhn.
  A CLI train-kuhn salva/retoma esses registros, inclusive após backup/restauração.
- Crenças bayesianas estáveis em log, Pareto, Kelly fracionário, VCG de resultados
  finitos e Brier; APIs de biblioteca com testes próprios.
- Quórum Ed25519 com n=3f+1 e 2f+1 assinaturas distintas, vinculado a digest,
  sequência, época e validade. É software BFT; não hardware-backed MinBFT.
- Diques imutáveis com TTL/topologia; planejador de isolamento bidirecional e decoys
  com caminho explícito para o operador. fire_dike aplica isolamento/fortificação
  só no banco local, por POST /actions; nenhum efeito sai do MachiavelliDB.
- Fortuna Pareto reproduzível e Centauro com rollouts mmap isolados e candidatos
  a diques. Esse tick continua separado do novo planejador.
- MCTS/UCT genérico com Max/Min/Chance, budgets de tempo/profundidade/memória e seed.
  CLI plan e PRINCE MCTS (REST/gRPC) executam um modelo harmônico de controles/choques,
  somente para recomendação. Testes cobrem armadilha adversarial e chance ponderada.
- ParticleFilter com propagação Verlet/Yoshida compensada, observações gaussianas/em
  log, ESS e reamostragem sistemática reproduzível. Previsões interrompidas ou inválidas
  não publicam estado parcial; Casimirs são preservados. A ação observe grava o
  posterior em cf_beliefs; com crença presente, PRINCE MCTS planeia sobre o ensemble
  com controlo partilhado e resolve por CFR+ os payoffs das arestas adjacentes.
  Não há ingestão automática de sensores; a dinâmica é o modelo de referência.
- PQL PEG com os 11 verbos e validação de orçamentos. SCOUT, WARGAME, PRINCE KUHN e PRINCE MCTS
  executam na API; os verbos táticos da PQL continuam a devolver conflito. A execução
  tática passa só por POST /actions, com autorização do Heraclitus.
- REST e gRPC nativos na mesma porta, token obrigatório, duas consultas PQL
  concorrentes, limites de entrada e paginação REST por UUID/sequence.
  POST /transactions confirma lotes de até 256 operações com sequência esperada;
  GET /strategic e /nodes/{id}/adjacency expõem os registros e o grafo persistidos.
  A API recusa posturas táticas e a escrita de arestas/diques pela rota de transações.
- Lo Stato WebGL local em /observatory, com posição/momento e risco por nó.
  Sintaxe JavaScript verificada; renderização, cobertura completa e 60fps não qualificados.
- Cliente gRPC Heraclitus Append com autenticação e idempotência, testado em mock local.
  Append comum não é recibo Merkle nem AdminIntent.
- Protocolo external-intent-v1 no HeraclitusDB 3.0.6 (PR #42 desse repositório; o
  MachiavelliDB 1.0.0-alpha.2 exige essa versão ou posterior): external-intent-v1,
  external-status-v1 e external-result-v1
  pelo RPC Admin, gravados com execute_admin/DurableIntent, isolados por tenant e
  principal, idempotentes e recusados para a mesma chave com outro conteúdo.
  O recibo traz intenção, principal, tenant e intent_lsn; não traz prova Merkle.
- CUDA: dois kernels, biblioteca hospedeira (cuda/host.cu) carregada por dlopen com
  ABI v1 na feature cuda (Linux), e fallback CPU determinístico. scripts/cuda.sh
  compila e compara com a CPU; não correu: o WSL atual não tem nvcc nem GPU.
  Sem a biblioteca, o código falha em vez de apresentar CPU como GPU.
- Lean 4.15: nove propriedades principais, mais lemas auxiliares, sem provas admitidas:
  identidades simpléticas, preservação por shears/composição, CFR+ não negativo e
  verdade no leilão de segundo preço. Modelo inteiro exato, sem refinamento f64.

## Validação integrada — 25/10/2026, tarde (Linux/WSL)

Conclusão do trabalho do Codex na Trinitas e validação só no Ubuntu-24.04 do WSL.
scripts/verify.sh --qualification, com MACHIAVELLI_E2E_HERACLITUS, passou os oito
gates: formatação, 168 testes do workspace com rocksdb-backend (0 falhas, 0
ignorados), Clippy -D warnings, build release, smoke, Heraclitus ponta a ponta,
CFR e arena. Evidência: /mnt/d/tmp/machiavelli-linux-verify-wEHCfwzm.

- Smoke Linux: autenticação, PQL Kuhn/WARGAME/MCTS sem mutação, BETRAY 409, fase
  inválida 422, lote com sequência e conflito, /actions 503 sem Heraclitus,
  SIGKILL e reabertura com sequência 2, exclusão durável pela CLI (sequência 3).
- Ponta a ponta com heraclitus-server real (build local com external-intent-v1):
  ALLY autorizado (recibo do principal, sequência 4), repetição idempotente, mesmo
  ID com outro conteúdo 409, Heraclitus parado com escrita bloqueada (409), SIGKILL
  do MachiavelliDB com a operação pendente e recuperação após os dois reinícios
  (aresta criada, sequência 7), escritas liberadas no fim.
- Trinitas: 9 testes, incluindo BETRAY (créditos, arestas, congelamento), dique
  (topologia, TTL, isolamento/fortificação, remoção), observações e expiração com e
  sem recibo remoto. heraclitus-server: teste external_intent e Clippy -D warnings.
- M2 Leduc: gap 0,000001737 em 133.407 iterações; Kuhn igual ao registado abaixo.
- M3 arena: mediana 2,2 µs, p99 54,9 µs com TMPDIR no ext4 do WSL. Em /mnt/d
  (drvfs) a mediana fica acima de 100 µs, por isso o gate usa $HOME/tmp e passou a
  falhar se a mediana ultrapassar 100 µs.
- CUDA: Clippy e testes com a feature cuda passam; cuda_matches_cpu_on_hardware
  fica ignorado e scripts/cuda.sh devolve 77 (sem nvcc/GPU). Nada qualificado em GPU.

Correções desta etapa: erro de compilação no coordenador (MutexGuard), avisos do
Clippy no storage/Trinitas/API, variante Neural em Box, /actions explícito sem
Heraclitus, smoke Linux equivalente ao do Windows e leitura de logs tolerante ao
ENODATA do drvfs. Não se recompilou no Windows; as provas Lean não mudaram e não
foram re-executadas nesta etapa (o verify.sh não tem gate Lean).

## Validação integrada — 25/10/2026, manhã (Windows + WSL)

scripts/verify.ps1 -RocksDB -Lean concluiu novamente os sete gates.
Foram 161 testes no workspace Windows; no WSL com rocksdb-backend, passaram
14 testes do storage, 12 do Centauro e 9 do Fog (35 ao todo). Clippy com
-D warnings, build, smoke REST/PQL/MCTS, gRPC autenticado e provas Lean passaram.
A consulta MCTS foi validada na CLI e nos dois protocolos, preservando a fase
e a sequência persistidas. A busca venceu a armadilha adversarial de referência;
isso não constitui qualificação de um domínio tático real.

O exemplo particle_filter executou previsão/observação/reamostragem: 17 partículas,
ESS 5,248742 antes da reamostragem e 17 depois; para observação de posição 0,3,
a média posterior foi 0,2999984104, com Casimir 7 inalterado.
Os testes também cobrem reversibilidade, evidência impossível e rollback numérico.
Consulte PLANNING.md para os modelos, comandos e limites dessa evidência.
## Validação integrada — 23/10/2026

scripts/verify.ps1 -RocksDB -Lean concluiu os sete gates: formatação, testes,
Clippy com -D warnings, build, smoke HTTP, RocksDB e Lean.
Foram 141 testes no workspace Windows e 14 testes do storage RocksDB no WSL,
sem falhas. O smoke confirmou autenticação, PQL, commit em lote, conflito de
sequência, recuperação após término do processo e exclusão durável pela CLI.
O teste RocksDB também injeta uma inconsistência no índice e confirma a recusa
na reabertura. Isso qualifica a alpha local; não os gates físicos de produção.

## Qualificação medida

| Gate | Resultado | Limite da evidência |
|---|---|---|
| M1 | 1.000.000 passos Verlet FPU n64; erro <1e-9 | modelo e passo de referência |
| M2 Kuhn | gap 0.000012641; 20.000 iterações | resultado numérico local |
| M2 Leduc | gap 0.000003031; 65.449 iterações em orçamento de 30s | resultado numérico local |
| M2 ataque/defesa | gap <1e-3 no teste de duas rotas | modelo pequeno explícito |
| M3 | 128 MB / 1M metadados; Windows: mediana 19,4 µs, p99 86,6 µs; WSL ext4: mediana 2,2 µs, p99 54,9 µs | amostra local; sem garantia de cauda em produção |
| M4–M5 | testes de planos, isolamento e disparo local de diques | sem efeito externo nem qualificação de produção |
| M6 | parcial | external-intent-v1 autoriza e fecha cada ação no Heraclitus (testado ponta a ponta); falta recibo Merkle verificável e papel dedicado |

## Trabalho que ainda impede chamar produção completa

1. Integrar a arena mmap ao armazenamento transacional; a cópia integral do mapa
   foi removida, mas a árvore persistente não substitui promoção de páginas mmap.
   O ledger registra resultados VCG verificados; ainda não transfere recursos reais.
2. Quórum e chaves: hoje vêm de um ficheiro do operador (MACHIAVELLI_QUORUM_FILE),
   sem rotação, persistência no banco nem roles. Créditos, BETRAY e diques são
   atómicos no banco local; nada disso move recursos fora dele.
3. Heraclitus: o external-intent-v1 saiu na 3.0.6. Falta criar um papel dedicado
   (hoje o token do Machiavelli precisa de Admin, que dá acesso a todas as
   operações administrativas); devolver e verificar prova Merkle do
   intent_lsn; suportar nós replicados (execute_admin recusa-se em cluster Raft).
   O Machiavelli confia no canal loopback e no token, não numa prova criptográfica.
4. Diques: o plano só dispara na sequência exata da criação +3; qualquer escrita
   intermédia invalida-o e exige recriação. Falta uma noção de topologia mais fina.
5. Modelos: ingestão automática de observações, modelos táticos de domínio,
   regret-based pruning, CE multijogador e Deep CFR em escala. A dinâmica do MCTS
   continua a ser o oscilador harmônico de referência.
6. CUDA em hardware (scripts/cuda.sh), HSM/MinBFT e gates de cauda.
7. Observabilidade, testes físicos de power-loss, performance/recuperação em escala
   e provas de refinamento Rust/f64.

Não se iniciou Heraclitus no Windows nem se alteraram os serviços Nietzsche/EVA.
O teste ponta a ponta usa uma instância descartável do Heraclitus no WSL (pasta
temporária, portas aleatórias, token gerado); o serviço heraclitus do operador
(:17474) não foi tocado. Os testes encerram seus próprios servidores.

## Formatos e limites

O WAL próprio usa frames MCV1 com cabeçalho de 80 bytes (magic, tamanho, sequência,
BLAKE3 do cabeçalho, BLAKE3 do envelope/payload); máximo 16 MiB por transação.
Somente caudas incompletas são truncadas. Corrupção completa falha sem reparo automático.
Erro ambíguo de escrita invalida o writer; reabra e confira antes de repetir.

RocksDB e WAL próprio usam pastas distintas, sem migração implícita. Abrir uma pasta
de outro backend é recusado. Backup v2 é snapshot portátil de nós e registros estratégicos; não preserva histórico
de transações nem configurações externas. Um backup/restauro incompleto fica no disco
para inspeção, sem marcador de conclusão.

O envelope de transação WAL agora é v2; a leitura de WAL e backups v1 permanece
compatível. Binários antigos não devem abrir bancos que receberam gravações novas.
O restore preserva o conteúdo dos diques, mas não promete que seu TTL/topologia
continue aplicável ao banco restaurado; planos antigos devem ser requalificados.
O ledger é append-only, com resultados conferidos pelo algoritmo VCG.
O outbox Trinitas (cf_trinitas_sync no RocksDB, registo Sync no WAL) guarda pedido,
operações planeadas, digest, recibo, resultado e acknowledged; só avança, nunca
reescreve. Cada ação consome três sequências (duas se nunca foi autorizada).
Caminhos REST desconhecidos devolvem 200 sem corpo, pelo fallback do router gRPC
partilhado; os clientes devem exigir o JSON esperado. POST /actions sem Heraclitus
configurado devolve 503.
Consulte STORAGE.md para formatos/operações e PLANNING.md para MCTS e partículas.

As limitações matemáticas do manifesto também importam: mínimo local não prova bacia
global/irreversibilidade; Kelly não garante risco de ruína zero no mundo real; numa
variedade simplética pura só há Casimirs constantes. O core usa extensão de Poisson
com coordenadas constitucionais. Provas em inteiros não certificam floats.

## Reprodução

Linux/WSL é o ambiente de execução e validação:

~~~bash
wsl -d Ubuntu-24.04 -e bash /mnt/d/DEV/MachiavelliDB/scripts/verify.sh --qualification
# Com o teste ponta a ponta contra um Heraclitus descartável:
wsl -d Ubuntu-24.04 -e bash -lc 'MACHIAVELLI_E2E_HERACLITUS=$HOME/cargo-target/heraclitus/debug/heraclitus-server bash /mnt/d/DEV/MachiavelliDB/scripts/verify.sh'
# CUDA (exige nvcc e GPU visível no WSL; sai com 77 se faltarem):
wsl -d Ubuntu-24.04 -e bash /mnt/d/DEV/MachiavelliDB/scripts/cuda.sh
~~~

Os logs de cada gate ficam em /mnt/d/tmp/machiavelli-linux-verify-*. O Windows
mantém os scripts antigos para edição local:

~~~powershell
Set-Location D:\DEV\MachiavelliDB
.\scripts\verify.ps1 -RocksDB -Lean
# Ou execute os gates individualmente:
.\scripts\cargo.ps1 fmt --all '--' --check
.\scripts\cargo.ps1 test --workspace --offline
.\scripts\cargo.ps1 clippy --workspace --all-targets --offline '--' -D warnings
.\scripts\cargo.ps1 build --workspace --offline
.\scripts\lean.ps1 build
.\scripts\smoke-api.ps1
.\scripts\cargo.ps1 run -p machiavelli-cfr --example cfr-qualification --offline
.\scripts\cargo.ps1 run -p machiavelli-cow-arena --example cow-arena-qualification --offline
~~~

RocksDB precisa de compilador C++/libclang. Foi compilado e testado no Ubuntu-24.04
em D:\WSL, com CARGO_TARGET_DIR=/home/junior/cargo-target/machiavelli e
LIBCLANG_PATH=/home/junior/cargo-target/machiavelli/clang.
Use scripts/rocks.ps1 para repetir os testes nativos.
