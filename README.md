# AGI Brain para Windows

[![Windows](https://github.com/caiomioto2/agi-brain/actions/workflows/windows.yml/badge.svg?branch=windows)](https://github.com/caiomioto2/agi-brain/actions/workflows/windows.yml)

Visualizador nativo do GBrain para **Windows 10/11 x64**, escrito em Rust.
Fork comunitário do [Glia de David Raphael](https://github.com/draphael89/glia),
com licença MIT e atribuição original preservadas. A versão macOS continua no
projeto original; a implementação Windows está na branch `windows` deste fork.

**Português brasileiro e Constelação são os padrões.**

## Instalar

Baixe `AGI-Brain-Windows-x64.zip` em [Releases](https://github.com/caiomioto2/agi-brain/releases),
extraia e execute `agi-brain.exe`. O pacote é portátil e não assinado.
Clique em **Explorar demonstração** para conhecer o app sem configurar um GBrain.

```powershell
.\agi-brain.exe --demo
.\agi-brain.exe --data C:\MeuGBrain\viz\graph.json
.\agi-brain.exe --live gbrain-aws
```

`gbrain-aws` é um exemplo: use o seu alias SSH configurado. SSH é o canal privado
que usa o acesso existente ao servidor; o aplicativo não cria um servidor AWS.

## Recursos

- **Constelação:** fundo escuro, pontos luminosos e crescimento gradual de novas
  páginas. Em **Aparência**, o estilo **Gephi** oferece fundo branco, pontos sólidos,
  tamanho por conexões e linhas coloridas. Os ajustes escolhidos são lembrados.
  O estilo Gephi não implementa ForceAtlas2 ou os plugins do Gephi.
- Cores estáveis por tipo, iguais no mapa, na legenda e no inspetor, inclusive
  para tipos personalizados. Foco em uma página destaca suas conexões.
- Busca por título, identificador, tipo e fonte (`Ctrl K`); tipos também podem
  ser buscados em português. Favoritos (`Ctrl D`), filtros e leitura de Markdown.
- Reprodução do crescimento por data de criação, com datas em dia/mês/ano.
  Usa o mapa atual; não reconstrói páginas apagadas nem o histórico de conexões.
- Posições persistentes e atualização sem perder câmera, seleção ou favoritos.
  Pontos aparecem antes das linhas, e novas páginas nascem perto dos vizinhos.
- Monitor local com `notify`: espera 180 ms após o último evento antes de ler o
  arquivo; interpretação e cálculo de posições ocorrem fora da interface.
  O app observa o export existente; não agenda exportações no GBrain.
- AWS privada, somente leitura, com verificações a cada 15 segundos por padrão,
  recuperação de conexão e **Detectar mudanças e atualizar o mapa**. O último
  mapa válido continua visível durante falhas. Esse acesso consulta periodicamente
  o servidor; não recebe notificações instantâneas enviadas pela AWS.
- Área de notificações do Windows e restauração da janela.
  A abertura normal e a restauração pela área de notificações sempre maximizam
  a janela, mesmo se uma versão anterior tiver salvo um tamanho reduzido.
- Exportação explícita de favoritos, seleção, identidade ou páginas disponíveis.
  Conteúdo hospedado é lido sob demanda e mantido em memória; mapa e posições
  ficam no armazenamento local. Exportações só são gravadas quando solicitadas.
- `agi-brain-context.exe`: servidor MCP — protocolo para outros agentes consultarem
  a identidade exportada e lerem páginas. Ferramentas: `health`, `who_am_i`,
  `recall`, `prime_context`, `explain_context`. Registro em agentes é explícito.

Conexões entre o mesmo par de páginas compartilham uma linha. A busca usa os
metadados do mapa. Abrir no Obsidian requer a pasta local correspondente.
Exportações locais leem Markdown em `<pasta GBrain>/source-<fonte>` ou nas pastas
escolhidas em Configurações.

## Conectar ao GBrain hospedado

Requer um servidor Linux existente com GBrain em PostgreSQL, Python 3, `psql`,
configuração em `~/.gbrain/config.json` e acesso SSH autorizado. O leitor incluído
usa o esquema PostgreSQL do GBrain (`pages`, `sources`, `links`); não é um leitor
PGLite. Credenciais permanecem no servidor.

A implantação atual usa `/root/.local/share/glia/read_only_bridge.py` e pressupõe
que o alias SSH entre como root, com o GBrain já configurado nessa conta.
Confira essa condição antes de implantar; nenhuma porta pública é necessária.

```powershell
& .\scripts\deploy-reader.ps1 -SshHost gbrain-aws
```

Depois, em **Abrir → Conectar ao GBrain na AWS**, informe o mesmo alias e clique
em **Conectar ao vivo**. Cada consulta usa uma transação somente de leitura.
O leitor aceita apenas mapa, páginas e saúde da conexão; não aceita SQL arbitrário.

## Configurar outro agente

Adapte o caminho à pasta em que extraiu o aplicativo:

```json
{
  "mcpServers": {
    "agi-brain-context": {
      "command": "C:\\Apps\\AGI Brain\\agi-brain-context.exe",
      "args": []
    }
  }
}
```

`who_am_i` lê a identidade exportada explicitamente. `recall` busca o mapa e lê
páginas locais ou hospedadas; `prime_context` combina identidade e páginas,
informando conteúdo indisponível. Nenhuma ferramenta grava no GBrain.

## Compilar e verificar

Requer Rust estável, Visual Studio C++ Build Tools e Windows SDK com `rc.exe`.
Use um terminal de desenvolvimento x64 ou o script que detecta o ambiente MSVC.
As dependências estão fixadas em `Cargo.lock`; `CARGO_TARGET_DIR` é respeitado.

```powershell
git clone --branch windows https://github.com/caiomioto2/agi-brain.git
cd agi-brain
& .\scripts\cargo.ps1 @('fmt', '--', '--check')
& .\scripts\cargo.ps1 @('clippy', '--locked', '--all-targets', '--', '-D', 'warnings')
& .\scripts\cargo.ps1 @('test', '--locked', '--all-targets')
& .\scripts\cargo.ps1 @('build', '--release', '--locked', '--bins')
& .\scripts\package.ps1
& .\scripts\verify-native.ps1 -Executable '.\dist\AGI Brain\agi-brain.exe'
& .\scripts\verify-context.ps1 -Executable '.\dist\AGI Brain\agi-brain-context.exe'
& .\scripts\verify-window.ps1 -Executable '.\dist\AGI Brain\agi-brain.exe'
```

Os testes hospedados são opcionais e usam exclusivamente seu servidor configurado:

```powershell
$env:GLIA_TEST_SSH_HOST = 'gbrain-aws'
& .\scripts\cargo.ps1 @('test', '--locked', '--test', 'live_aws', '--', '--ignored', '--nocapture')
& .\scripts\verify-native.ps1 -Executable '.\dist\AGI Brain\agi-brain.exe' -LiveHost gbrain-aws -LivePageSlug 'notes/sua-pagina'
& .\scripts\verify-context.ps1 -Executable '.\dist\AGI Brain\agi-brain-context.exe' -LiveHost gbrain-aws
```

A verificação nativa precisa de uma sessão gráfica Windows. Para repetir o teste
completo do padrão inicial, use um novo `-OutputDirectory`. Não altera as
preferências pessoais. Veja [VALIDATION.md](VALIDATION.md) para a cobertura.

Configurações ficam em `%LOCALAPPDATA%/Glia`, e a identidade explícita em
`~/.glia/psyche.md`. Os nomes `GLIA_STATE_DIR`, `GLIA_DATA`, `GLIA_PSYCHE` e
`GLIA_SNAPSHOT*` mantêm compatibilidade. O JSON padrão é `~/.gbrain/viz/graph.json`
e aceita o formato [documentado no Glia](https://github.com/draphael89/glia#data).

## Contribuir e licença

Issues e propostas de alteração são bem-vindas neste fork. Inclua o problema,
a mudança de comportamento e a verificação executada. Use dados sintéticos para
reprodução; não inclua credenciais, páginas pessoais ou configuração do servidor.
A branch `main` preserva o projeto macOS original; contribuições Windows usam
`windows`. Consulte [docs/live-map-spec.md](docs/live-map-spec.md).

[MIT](LICENSE). Copyright original: David Raphael, 2026. Este fork comunitário
não implica suporte ou endosso do autor original. O símbolo do app foi fornecido
pelo responsável pelo fork e ampliado com imagegen; veja [a procedência](assets/icon-provenance.md).
