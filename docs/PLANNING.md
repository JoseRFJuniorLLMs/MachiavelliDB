# Planejamento MCTS e filtro de partículas

## Consulta MCTS pelo operador

~~~powershell
Set-Location D:\DEV\MachiavelliDB
.\scripts\cargo.ps1 build --workspace --offline
D:\cargo-target\debug\machiavelli.exe D:\tmp\machiavelli-demo plan 6a54d3ce-57db-476e-bc63-4d26baf20450 2000 8 42 1000
~~~

O nó precisa existir na pasta usada. Os argumentos de plan são ID, iterações,
horizonte, semente e deadline em milissegundos. A CLI abre a pasta com o lock
usual; pare o servidor dessa pasta antes de usá-la.

Para planejar com o servidor ativo, envie POST /pql com o token de autenticação:

~~~json
{"query":"PRINCE MCTS 6a54d3ce-57db-476e-bc63-4d26baf20450 ITERATIONS 2000 HORIZON 8 SEED 42 DEADLINE 1000"}
~~~

A mesma consulta funciona pelo RPC Query. São aceitas 1..100.000 iterações,
horizonte 1..32 e deadline 1..10.000 ms. O nó deve ter dimensão 1..128 e até
128 coordenadas constitucionais. A árvore guarda no máximo 4.096 estados;
ao atingir esse limite, continua com rollouts sem expandir novos nós.
A API compartilha as duas vagas de solver com CFR/WARGAME; a vaga da busca
continua ocupada no worker mesmo se o cliente cancelar a requisição.

A resposta informa source_sequence, persisted=false, model, horizon, seed e search:

- selected_action: hold, damp ou retreat no modelo de referência.
- iterations: simulações completas aproveitadas; uma interrompida não entra na média.
- actions: visitas e mean_value por ação; null indica ausência de amostras.
- root_value: valor médio observado, na perspectiva do jogador Max.
- nodes, depth_cutoffs, node_limit_reached e deadline_reached: limites efetivamente atingidos.

Uma consulta sem simulações completas retorna a primeira ação legal como fallback,
com zero visitas e médias null. O operador deve inspecionar essa evidência.
O valor não é uma prova de ótimo ou certificado de explorabilidade.

## Motor UCT e modelo disponível

machiavelli_centaur::mcts expõe Model, Position, Config e search. O modelo fornece
transições determinísticas e posições Decision(Max/Min), Chance ou Terminal.
Chance tem probabilidades explícitas e é amostrada por elas, sem maximizar seus
resultados. UCT inverte o sinal da média nos turnos Min; todas as recompensas
terminais e avaliações de corte devem estar em [-1,1]. A expansão valida ações,
probabilidades e valores. A semente torna repetições do mesmo orçamento de
iterações reproduzíveis; interrupções por tempo podem completar quantidades diferentes.
O prazo é cooperativo, portanto callbacks de modelos externos também precisam
ter custo limitado.

O adaptador planning usa um Hamiltoniano harmônico com a massa do nó. A cada
etapa há uma escolha entre manter, amortecer o momento ou retrair o estado,
seguida de choques de momento negativo/nulo/positivo com probabilidades
0,2/0,6/0,2. Os custos 0/0,03/0,08 e a amplitude 0,1+fortuna_exposure são parâmetros
de referência. A integração Verlet ocorre entre controles; controles dissipativos
não tornam o ciclo completo um fluxo simplético conservativo. A recompensa é
o negativo de risco energético normalizado mais custos, limitado a [-1,0].

Esse adaptador resolve um modelo totalmente observado com choques explícitos.
Não resolve jogos táticos reais, não acopla MCTS a CFR e não agrega estados ocultos
em information sets. Recomendações não disparam BETRAY/FORTIFY, não gravam nós
ou diques e não representam autorização administrativa. O tick de risco mmap
continua disponível separadamente. O modelo de produção deve definir suas ações,
utilidades e observações antes de substituir esse adaptador.

Os testes incluem uma armadilha em dois turnos em que a ação aparentemente lucrativa
é punida pelo adversário; a busca prefere o ramo seguro. Também verificam a
frequência de eventos de chance, eventos de peso zero, memória limitada, ciclos,
prazo esgotado e reprodutibilidade. CLI, REST e gRPC verificam preservação do banco.

## Filtro de partículas do Fog of War

machiavelli_fog::ParticleFilter representa até 4.096 hipóteses PhaseVector de
mesma dimensão e mesma constituição, com pesos bayesianos. A dimensão e a
quantidade de Casimirs são limitadas a 128. O exemplo executável usa somente memória:

~~~powershell
.\scripts\cargo.ps1 run -p machiavelli-fog --example particle_filter --offline
~~~

O fluxo da biblioteca é:

1. new(partículas, pesos, semente) valida estados, formas e evidência inicial.
2. predict(Hamiltoniano separável, Scheme, dt, passos, deadline) propaga um ensemble
   candidato com integradores compensados, em blocos de até 64 passos. Publica todas
   as partículas juntas. Retorna false se o prazo descartar a previsão; erro numérico
   também mantém o ensemble e os pesos anteriores. Aceita até 1.000.000 passos.
3. observe_position(observação, desvio) aplica um sensor gaussiano isotrópico de posição.
   observe_log_likelihoods permite outros sensores. Evidência impossível é recusada
   sem perder o prior, usando a atualização em log existente.
4. effective_sample_size informa ESS=1/soma(peso²).
5. resample_if_needed(fração) faz reamostragem sistemática se ESS < fração*N,
   com tolerância de arredondamento e semente reproduzível. Os pesos voltam a uniformes.
6. mean_phase calcula a média ponderada de q/p e preserva a constituição. A escala
   por coordenada evita overflow em médias entre extremos finitos de sinais opostos.

A propagação usa mapas simpléticos por partícula. Observação e reamostragem são
operações estatísticas e não preservam volume de Liouville. O filtro não inventa
ruído de processo ou rejuvenescimento; modelos de domínio podem precisar deles
para evitar perda de diversidade após muitas reamostragens.

## Crenças persistidas e planejamento sob informação imperfeita

A ação tática observe (POST /actions, ver QUICKSTART.md) aplica uma observação de
posição ao ensemble e grava o posterior em cf_beliefs, no mesmo commit autorizado.
A primeira observação exige partículas explícitas como prior; as seguintes partem
do posterior gravado e precisam de observed_at_ms maior. Não há ingestão automática
de sensores: cada observação entra como um pedido autorizado.

Se o nó tiver crença, PRINCE MCTS planeia sobre o ensemble inteiro (até 256
partículas): cada decisão é partilhada por todas as hipóteses, e o valor é a média
ponderada pelos pesos. A identidade da partícula nunca vira um ramo de decisão.
A resposta passa a model posterior_shared_control_harmonic e traz observation_id.
As matrizes de payoff das arestas adjacentes (até 16, até 32x32) são resolvidas por
CFR+ e devolvidas em games. A dinâmica continua a ser o modelo harmônico de
referência; não é um modelo tático de domínio qualificado.
