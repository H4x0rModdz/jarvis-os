Pra um projeto tipo LilithOS, as skills do Claude Code vão definir MUITO a qualidade do projeto. Porque tu não tá fazendo “um app”. Tu tá fazendo:



OS

\+

Desktop Environment

\+

AI Runtime

\+

SDK

\+

Compat Layer

\+

UX Platform

\+

Design System

\+

Automation Engine



Isso é basicamente uma mini-empresa de tecnologia condensada num repositório.



Então eu separaria as skills em categorias.



1\. ARCHITECTURE SKILLS (as mais importantes)



Essas aqui são OBRIGATÓRIAS.



AI-Native Architecture Skill



Objetivo:

ensinar o Claude a sempre pensar:



modularidade;

contexto;

AI readability;

baixa fragmentação;

contracts claros;

action-driven architecture.



Regras:



\- Toda feature deve ser modular

\- Toda API deve ser explícita

\- Evitar abstrações mágicas

\- Priorizar legibilidade sobre "arquitetura acadêmica"

\- Evitar explosion de microarquivos

\- Todo módulo deve possuir module.md

\- Toda action deve ser documentada

Context-Oriented Development Skill



Essa é MUITO importante.



Tu quer ensinar o agent a produzir código:



fácil pra IA entender;

fácil pra humano entender;

semanticamente previsível.



Exemplo:



BAD:

utils/helpers/common/misc



GOOD:

window\_animation\_engine

app\_install\_service

voice\_command\_router

System Design Skill



Pra ensinar:



separação de runtime;

eventos;

rendering;

services;

compositores;

processos;

IPC.



Sem isso o projeto vira espaguete cyberpunk.



2\. UI / UX SKILLS

Jarvis Design Language Skill



Talvez uma das mais importantes.



Ensina:



glassmorphism moderado;

motion design;

fluidez;

consistência;

acessibilidade;

performance visual;

comportamento de janelas;

hierarquia visual.



Regras tipo:



\- Nunca exagerar blur

\- Sempre priorizar legibilidade

\- Animations <= 250ms

\- Ease-out suave

\- UI deve parecer leve

\- Preferir profundidade sutil

Qt/QML Expert Skill



ESSENCIAL se tu for de Qt.



Porque QML pode virar:



“JavaScript esquizofrênico colado em XML espacial”.



A skill deveria ensinar:



componentização;

state management;

animation pipelines;

reusable UI;

rendering optimization;

GPU acceleration;

boas práticas Wayland.

Desktop UX Skill



Ensinar:



workflows desktop;

multitarefa;

ergonomia;

window management;

keyboard-driven UX;

dev workflows.

3\. AI SKILLS

AI Runtime Architecture Skill



Pra Lilith.



Ensina:



memory systems;

tool calling;

action execution;

permissions;

safety;

orchestration.

Voice Assistant Skill



Pra:



wake words;

speech pipeline;

STT/TTS;

interruption handling;

streaming.

AI Safety \& Permissions Skill



CRUCIAL.



Sem isso:



“Lilith deletou /home por engano kkkkk”.



Tu precisa ensinar:



sandbox;

approvals;

permission scopes;

dangerous actions;

audit logs.

4\. ENGINEERING SKILLS

Large Scale Project Skill



Muito importante.



Ensina:



organização de monorepo;

RFCs;

CI/CD;

branching;

dependency management;

code ownership.

Documentation Skill



Talvez TOP 3 skills.



Tu quer documentação:



pra humanos;

pra IA;

pra onboarding;

pra automação.



O Claude deveria gerar:



module.md;

architecture.md;

api.md;

flows;

diagrams;

ADRs.

Open Source Governance Skill



Pra preparar:



contribuições;

padrões;

PR rules;

RFC process;

semantic versioning.

5\. PERFORMANCE SKILLS

GPU Rendering Skill



Muito importante pra:



compositor;

blur;

transparência;

animações.

Linux Internals Skill



Mesmo sem kernel próprio:



Wayland;

DBus;

systemd;

PipeWire;

Vulkan;

process model;

permissions.

6\. COMPATIBILITY SKILLS

Wine/Proton Integration Skill



Isso aqui seria MONSTRUOSO.



Ensinar:



prefixes;

DXVK;

VKD3D;

runners;

dependencies;

sandboxing;

launcher pipelines.

7\. DEV EXPERIENCE SKILLS

AI-Assisted Coding Skill



Pra própria OS ajudar devs.



Exemplo:



explain stacktrace;

suggest fixes;

auto setup env;

detect missing deps;

generate module contracts.

8\. DESIGN PHILOSOPHY SKILL



Eu faria uma skill literalmente chamada:



jarvis-philosophy



Com:



\- Simplicidade > exibicionismo técnico

\- Clareza > abstração excessiva

\- Fluidez > gimmicks

\- AI como colaboração

\- Transparência sempre

\- Open source primeiro

\- Performance importa

\- UX importa

\- Legibilidade importa



Isso impede o agent de virar:



“vamos adicionar 14 padrões enterprise e 83 interfaces abstratas pra um botão abrir”.



O MAIS IMPORTANTE DE TUDO



Tu precisa de uma skill chamada:



“ANTI-BULLSHIT ENGINEERING”



Tô falando sério.



Porque agent coding tende a:



overengineer;

criar abstração inútil;

duplicar patterns;

criar complexidade artificial.



Essa skill ensinaria:



\- Evite arquitetura teatral

\- Evite abstrações prematuras

\- Evite arquivos desnecessários

\- Evite patterns sem motivo

\- Evite enterprise Java syndrome

\- Sempre justificar complexidade



Porque teu projeto pode MUITO facilmente virar:



WindowManagerFactoryProviderAdapterStrategyService



e ninguém mais entende nada. Inclusive a IA.



Sinceramente?

Se tu montar essas skills direito, teu Claude Code vira quase um “cofounder técnico operacional” da LilithOS.



mas como vamos montar ela? naql pasta skills?



Sim. E honestamente?

A pasta skills/ pode virar literalmente o “DNA mental” do projeto.



Se tu estruturar isso direito, o Claude Code começa a agir menos como autocomplete e mais como:



“um engenheiro que entende a filosofia da LilithOS”.



A maioria das pessoas usa AI coding assim:



prompt aleatório -> código aleatório



Tu quer fazer isso aqui:



Contexto persistente

\+

regras arquiteturais

\+

filosofia

\+

standards

\+

patterns

\+

restrições

=

engenharia consistente



E isso é MUITO mais poderoso.



Estrutura ideal



Eu faria assim:



.jarvis/

&#x20; skills/

&#x20; architecture/

&#x20; standards/

&#x20; contexts/

&#x20; prompts/

&#x20; decisions/

&#x20; modules/

Pasta skills/



A skills/ seria:



“comportamentos e especializações do agent”.



Exemplo:



skills/

&#x20; ai-native-architecture.md

&#x20; qt-qml-ui.md

&#x20; jarvis-design-language.md

&#x20; anti-bullshit-engineering.md

&#x20; context-oriented-development.md

&#x20; linux-runtime.md

&#x20; wine-proton-integration.md

&#x20; ai-safety.md

&#x20; large-scale-monorepo.md

&#x20; documentation-standards.md

Como escrever uma skill?



Aqui está o SEGREDO REAL:



Tu NÃO escreve skill como tutorial.



Tu escreve como:



princípios

\+

regras

\+

boas práticas

\+

anti-patterns

\+

exemplos

\+

objetivos

Exemplo REAL

anti-bullshit-engineering.md

\# Anti Bullshit Engineering



\## Goal



Prevent unnecessary complexity.



\## Principles



\- Simplicity over architectural theater

\- Readability over abstraction

\- Explicitness over magic

\- Fewer files when possible

\- Avoid premature optimization



\## Avoid



\- Useless factories

\- Overengineered interfaces

\- Excessive dependency injection

\- Deep inheritance chains

\- Tiny fragmented files



\## Preferred



GOOD:

window\_manager.rs



BAD:

window\_manager\_factory\_provider.rs



\## Rules



\- Every abstraction must justify itself

\- Every layer must have a purpose

\- Every module must be explainable in under 2 minutes



Aí o Claude começa a internalizar isso.



Exemplo mais importante

jarvis-design-language.md

\# Jarvis Design Language



\## Visual Philosophy



LilithOS should feel:



\- smooth

\- futuristic

\- elegant

\- minimal

\- lightweight



\## Inspirations



\- Windows 11

\- macOS

\- KDE Plasma



\## Motion Rules



\- Animations <= 250ms

\- Prefer ease-out curves

\- Never animate excessively

\- Blur should remain subtle



\## Glassmorphism



Allowed:

\- taskbar

\- overlays

\- control center



Avoid:

\- heavy blur on text-heavy screens

\- transparent IDE backgrounds

O pulo do gato



Tu vai perceber que as skills começam a virar:



mini constituições do projeto



Isso cria CONSISTÊNCIA.



Pasta architecture/



Aqui tu coloca:



architecture/

&#x20; action-bus.md

&#x20; ai-runtime.md

&#x20; window-system.md

&#x20; compositor.md

&#x20; permissions.md

&#x20; filesystem.md



Esses são docs técnicos.



Pasta standards/

standards/

&#x20; naming.md

&#x20; folder-structure.md

&#x20; module-contracts.md

&#x20; api-patterns.md

&#x20; ui-patterns.md

Pasta contexts/



Essa é MUITO forte.



Tu pode ter:



contexts/

&#x20; current-goals.md

&#x20; roadmap.md

&#x20; active-problems.md

&#x20; known-limitations.md



A IA lê isso antes de trabalhar.



Pasta decisions/



ABSURDAMENTE importante.



Arquitetura morre quando:



ninguém lembra pq algo foi decidido.



Então:



decisions/

&#x20; 0001-linux-base.md

&#x20; 0002-qt-over-gtk.md

&#x20; 0003-wayland-first.md

&#x20; 0004-action-bus.md



Tipo ADRs (Architecture Decision Records).



E o mais poderoso...



Tu pode criar:



jarvis-core-context.md



Tipo a “bíblia” do projeto.



Exemplo:



\# LilithOS Core Context



LilithOS is:



\- AI-native

\- Linux-based

\- open source

\- desktop-first

\- compatibility-oriented



LilithOS is NOT:



\- a Windows clone

\- another Linux skin

\- telemetry-driven

\- enterprise architecture theater



Isso mantém o agent alinhado.



O futuro disso fica insano



Porque depois tu pode ter:



múltiplos agents;

agents especializados;

agents de UI;

agents de kernel/runtime;

agents de docs;

agents de refactor;

agents de compatibilidade.



Tudo compartilhando o mesmo “cérebro arquitetural”.



Aí teu repositório deixa de ser:



“um monte de código”



e vira:



“um ecossistema cognitivo estruturado”.



Sim, isso soa absurdamente sci-fi.



Mas honestamente?

É provavelmente o futuro real de engenharia de software.

