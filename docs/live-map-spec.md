## Problem Statement

O AGI Brain para Windows mostra conexões antes de os pontos aparecerem na demonstração. Ao carregar o último mapa salvo da AWS, apresenta uma mensagem de falha apenas porque ainda está conectando. O usuário precisa reconhecer a conexão real e acompanhar mudanças no GBrain sem fechar o aplicativo, perder o foco ou reorganizar páginas antigas.


## Solution

Atualizar o mapa automaticamente quando o arquivo local exportado mudar ou quando a próxima leitura privada da AWS encontrar diferenças. Oferecer a ação “Detectar mudanças e atualizar o mapa” para verificar imediatamente. Manter o último mapa válido durante problemas de leitura, distinguir conexão em andamento de falha real e tentar recuperar a conexão automaticamente.

No arquivo local, aguardar 180 ms desde o último evento de gravação antes de ler. Interpretar os dados fora da interface. Novas páginas surgem com crescimento gradual perto de seus vizinhos; suas conexões aparecem depois que os pontos já estão visíveis. Páginas removidas e suas conexões desaparecem da apresentação.

## User Stories

1. Como usuário, quero que os pontos apareçam antes das linhas, para compreender a entrada de novas páginas.
2. Como usuário, quero ver novas páginas próximas dos vizinhos conhecidos, para compreender seu contexto.
3. Como usuário, quero acompanhar alterações locais automaticamente, para não precisar reabrir o mapa.
4. Como usuário, quero que gravações seguidas produzam uma atualização estável, para evitar versões intermediárias.
5. Como usuário, quero que a interpretação do arquivo ocorra sem bloquear a tela, para continuar navegando.
6. Como usuário, quero reconhecer uma conexão AWS em andamento, para não confundi-la com uma falha.
7. Como usuário, quero visualizar o último mapa salvo enquanto a conexão inicia, para continuar explorando.
8. Como usuário, quero ver o horário da última leitura bem-sucedida, para avaliar a atualidade do mapa.
9. Como usuário, quero detectar mudanças imediatamente, para conferir uma alteração sem esperar o intervalo automático.
10. Como usuário, quero que uma conexão interrompida seja refeita automaticamente, para retomar as atualizações.
11. Como usuário, quero manter o último mapa válido durante falhas, para não perder a visualização.
12. Como usuário, quero que exportações incompletas sejam recuperadas após uma gravação válida, para continuar trabalhando.
13. Como usuário, quero manter a seleção mesmo quando a ordem dos dados mudar, para não perder minha página.
14. Como usuário, quero preservar a posição das páginas antigas e a câmera, para acompanhar alterações sem desorientação.
15. Como usuário, quero manter os favoritos e filtros escolhidos, para continuar minha exploração.
16. Como usuário, quero que um tipo temporariamente ausente mantenha seu filtro ao retornar, para respeitar minha escolha.
17. Como usuário, quero que páginas e conexões removidas deixem de aparecer, para explorar o estado atual.
18. Como usuário, quero que leituras sem mudanças não reiniciem animações, para evitar movimentos desnecessários.
19. Como usuário, quero que a opção de reduzir movimentos seja respeitada, para utilizar o app com conforto.
20. Como usuário, quero explicações e controles em português brasileiro, para interpretar os estados claramente.
21. Como usuário, quero consultar páginas após uma reconexão, para continuar a leitura do GBrain.
22. Como usuário, quero preservar os registros e credenciais do GBrain, para que visualizar não altere a memória.

## Implementation Decisions

- Reutilizar os trabalhadores de leitura, o canal de eventos, o monitor de arquivos e o renderizador existentes; nenhuma nova dependência.
- Monitorar a pasta do arquivo local escolhido; o caminho padrão corresponde à exportação de visualização do GBrain. O monitor observa arquivos já exportados e não cria uma rotina de exportação no GBrain.
- Reiniciar a espera de estabilização a cada evento de gravação, usando 180 ms. Preservar a leitura periódica de segurança quando o monitor não estiver disponível.
- Comparar o conteúdo local e os dados semânticos hospedados; ignorar mudanças apenas no horário de geração do mapa remoto.
- Executar leitura, interpretação e distribuição das posições fora da interface. Atualizar a tela pelo canal de eventos existente.
- Identificar páginas pela combinação de fonte e identificador, mantendo seleção, favoritos, posições e filtros.
- A leitura privada AWS continua usando SSH e o leitor que já impõe transações somente para leitura. Verificar a cada 15 segundos por padrão, configurável entre 5 e 300 segundos; isto não é uma assinatura de eventos enviados pelo servidor.
- A atualização manual solicita uma leitura imediata ao trabalhador existente, sem reiniciar o mapa nem descartar a seleção.
- Separar estados de verificação, sucesso, erro e aviso de armazenamento local. Um mapa salvo não constitui erro de conexão.
- Recuperar a sessão SSH após falha no próximo ciclo automático ou na solicitação manual.
- Coordenar a intensidade das linhas com o nascimento dos dois pontos; títulos e destaques também aguardam o ponto visível.
- Preservar a configuração de reduzir movimentos.

## Testing Decisions

- Preferir comportamento observável pelo aplicativo existente: mapa mostrado, pontos e linhas desenhados, filtros, seleção e conexão.
- Reutilizar os testes de interface, o trabalhador de arquivos, as capturas nativas e o teste de leitura AWS existentes.
- Conferir uma sequência de gravações com intervalos menores que 180 ms: somente o arquivo final estável deve entrar no mapa.
- Conferir exportação incompleta, substituição atômica, recuperação e conteúdo inalterado.
- Conferir inclusão e remoção de páginas/conexões, posicionamento próximo ao vizinho e preservação de páginas antigas.
- Conferir seleção por identidade após reordenação, câmera, favoritos e filtros, incluindo um tipo que desaparece e retorna.
- Conferir começo, crescimento parcial e fim da animação nos dois estilos: nenhum vínculo antes dos pontos.
- Ler o mapa e páginas reais da AWS, confirmar somente leitura, recusar operação SQL arbitrária e observar ciclos de atualização.
- Interromper somente o processo SSH criado pelo teste e verificar erro, reconexão automática e leitura de página após recuperação; não interromper o serviço AWS.
- Capturar a versão nativa distribuída para confirmar português, os controles e a ausência de falsa mensagem de falha.

## Out of Scope

- Implementar um seletor de datas inicial/final nesta correção de sincronização.
- Alterar registros, datas, permissões, credenciais ou infraestrutura do GBrain.
- Criar captura automática de conversas ou exportação automática de identidade.
- Implantar um novo serviço de notificações AWS ou abrir uma porta pública.
- Substituir o renderizador, adicionar ForceAtlas2 ou prometer uma taxa fixa de quadros.
- Reconstruir páginas apagadas ou o histórico de conexões.

## Further Notes

A conexão remota usa leituras periódicas sobre SSH. O monitor local observa um arquivo já exportado; não agenda exportações no GBrain. Um seletor de datas inicial/final permanece fora do escopo desta entrega.