#!/usr/bin/env node

/**
 * Servidor MCP de Stellar Game Studio.
 *
 * Deja que un asistente use el studio sin que nadie escriba pegamento: listar
 * contratos, crear un juego, compilar, generar bindings y desplegar.
 *
 * Cada herramienta corre el mismo comando que correria una persona, asi que no
 * hay una segunda implementacion que se desincronice cuando cambien los scripts.
 *
 * Corre en Node, no en Bun, aunque el repositorio use Bun: `scripts/utils/*`
 * depende de `Bun.TOML` y `Bun.file`, que no existen fuera de Bun. El servidor
 * lanza `bun run ...` como proceso hijo y para eso necesita saber donde esta el
 * binario, porque no suele estar en el PATH que hereda un servidor MCP.
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js'
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js'
import {
  CallToolRequestSchema,
  ListToolsRequestSchema
} from '@modelcontextprotocol/sdk/types.js'
import { spawn } from 'node:child_process'
import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'

// --------------------------------------------------------------------------
// Configuracion
// --------------------------------------------------------------------------

function flag(nombre: string): string | undefined {
  const i = process.argv.indexOf(nombre)
  return i !== -1 ? process.argv[i + 1] : undefined
}

const RAIZ = resolve(flag('--repo-root') || process.env.SGS_ROOT || process.cwd())
const BUN = flag('--bun') || process.env.SGS_BUN || 'bun'

/** Un build en frio compila todo soroban-sdk: quince minutos es realista. */
const LENTO = 900_000
const MEDIO = 180_000

// --------------------------------------------------------------------------
// Redaccion
// --------------------------------------------------------------------------

/**
 * Saca de cualquier texto lo que no puede salir del proceso.
 *
 * `.env` guarda claves secretas de Stellar en texto plano, y `deploy` las pasa
 * por linea de comando. Esto se aplica a stdout, a stderr y a los mensajes de
 * error, sin excepcion y sin bandera para desactivarlo: una clave secreta que
 * llega al historial de un asistente ya se filtro.
 */
function limpiar(texto: string): string {
  return texto
    .replace(/\bS[A-Z2-7]{55}\b/g, 'S…REDACTADO')
    .replace(/(--source-account\s+)\S+/g, '$1<REDACTADO>')
    .replace(/([A-Z0-9_]*SECRET[A-Z0-9_]*\s*=\s*)\S+/g, '$1<REDACTADO>')
}

// --------------------------------------------------------------------------
// Ejecucion
// --------------------------------------------------------------------------

type Resultado = { ok: boolean; salida: string }

function correr(args: string[], timeout: number): Promise<Resultado> {
  return new Promise((cumplir) => {
    const proc = spawn(BUN, args, { cwd: RAIZ, env: process.env, shell: false })

    let salida = ''
    const juntar = (d: Buffer) => {
      salida += d.toString()
      if (salida.length > 40_000) salida = salida.slice(-40_000)
    }
    proc.stdout.on('data', juntar)
    proc.stderr.on('data', juntar)

    const reloj = setTimeout(() => {
      proc.kill('SIGTERM')
      cumplir({ ok: false, salida: limpiar(salida) + `\n\n[cortado a los ${timeout / 1000}s]` })
    }, timeout)

    proc.on('close', (code) => {
      clearTimeout(reloj)
      cumplir({ ok: code === 0, salida: limpiar(salida) })
    })

    proc.on('error', (e) => {
      clearTimeout(reloj)
      cumplir({
        ok: false,
        salida:
          `No pude ejecutar "${BUN}": ${e.message}\n\n` +
          `bun no suele estar en el PATH que hereda un servidor MCP. ` +
          `Pasale la ruta absoluta con --bun /ruta/a/bun.`
      })
    })
  })
}

// --------------------------------------------------------------------------
// Lectura del repositorio
// --------------------------------------------------------------------------

/** Los nombres que create.ts no valida y que romperian el repositorio. */
const RESERVADOS = new Set([
  'dice-duel',
  'number-guess',
  'twenty-one',
  'mock-game-hub',
  'game-hub-core'
])

function slugValido(s: unknown): s is string {
  return typeof s === 'string' && /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/.test(s) && s.length <= 40
}

function contratos(): string[] {
  const dir = join(RAIZ, 'contracts')
  if (!existsSync(dir)) return []
  return readdirSync(dir, { withFileTypes: true })
    .filter((e) => e.isDirectory() && existsSync(join(dir, e.name, 'Cargo.toml')))
    .map((e) => e.name)
    .sort()
}

function desplegados(): Record<string, string> {
  const salida: Record<string, string> = {}
  const dj = join(RAIZ, 'deployment.json')
  if (existsSync(dj)) {
    try {
      const d = JSON.parse(readFileSync(dj, 'utf8'))
      if (d?.contracts) Object.assign(salida, d.contracts)
    } catch {
      /* si esta corrupto, caemos al .env */
    }
  }
  const env = join(RAIZ, '.env')
  if (existsSync(env)) {
    for (const linea of readFileSync(env, 'utf8').split('\n')) {
      const m = linea.match(/^(VITE_[A-Z0-9_]*CONTRACT[A-Z0-9_]*)\s*=\s*(.+)$/)
      if (m && !salida[m[1]]) salida[m[1]] = m[2].trim()
    }
  }
  return salida
}

// --------------------------------------------------------------------------
// Herramientas
// --------------------------------------------------------------------------

const HERRAMIENTAS = [
  {
    name: 'sgs_doctor',
    description:
      'Comprueba que este todo lo que hace falta: bun, cargo, el target ' +
      'wasm32v1-none, y los archivos del repositorio. Empeza por aca. El paquete ' +
      'se instala con npx pero la cadena de herramientas de abajo no viene adentro.',
    inputSchema: { type: 'object', properties: {} }
  },
  {
    name: 'sgs_list_contracts',
    description:
      'Lista los contratos del workspace y sus IDs desplegados, si los hay. ' +
      'No modifica nada.',
    inputSchema: { type: 'object', properties: {} }
  },
  {
    name: 'sgs_create',
    description:
      'Crea un juego nuevo: contrato Soroban y frontend propio. No despliega. ' +
      'Escribe archivos nuevos y modifica el Cargo.toml de la raiz.',
    inputSchema: {
      type: 'object',
      properties: {
        slug: { type: 'string', description: 'minusculas, numeros y guiones. Ej: "coin-flip"' },
        force: {
          type: 'boolean',
          description: 'BORRA el juego si ya existe, incluido su directorio entero. Por defecto false.'
        }
      },
      required: ['slug']
    }
  },
  {
    name: 'sgs_build',
    description:
      'Compila los contratos a wasm32v1-none. Sin argumentos compila todos. ' +
      'En frio tarda varios minutos.',
    inputSchema: {
      type: 'object',
      properties: {
        contracts: { type: 'array', items: { type: 'string' } }
      }
    }
  },
  {
    name: 'sgs_test',
    description: 'Corre los tests de los contratos. No toca la cadena ni escribe configuracion.',
    inputSchema: {
      type: 'object',
      properties: {
        contracts: { type: 'array', items: { type: 'string' } }
      }
    }
  },
  {
    name: 'sgs_bindings',
    description:
      'Genera los bindings de TypeScript que el frontend usa para invocar los ' +
      'contratos. Necesita que ya esten desplegados.',
    inputSchema: {
      type: 'object',
      properties: {
        contracts: { type: 'array', items: { type: 'string' } }
      }
    }
  },
  {
    name: 'sgs_deploy',
    description:
      'DESPLIEGA a testnet y reescribe el .env. Exige confirm: true. ' +
      'El .env se regenera desde plantilla, asi que se pierden las variables que ' +
      'no esten en ella. Pregunta antes de llamar a esto.',
    inputSchema: {
      type: 'object',
      properties: {
        contracts: { type: 'array', items: { type: 'string' } },
        confirm: { type: 'boolean', description: 'tiene que ser true' }
      },
      required: ['confirm']
    }
  },
  {
    name: 'sgs_publish',
    description: 'Exporta el frontend de un juego como build independiente. No despliega contratos.',
    inputSchema: {
      type: 'object',
      properties: {
        slug: { type: 'string' },
        build: { type: 'boolean' }
      },
      required: ['slug']
    }
  }
]

// --------------------------------------------------------------------------

const server = new Server(
  { name: 'stellar-game-studio', version: '0.1.0' },
  { capabilities: { tools: {} } }
)

server.setRequestHandler(ListToolsRequestSchema, async () => ({ tools: HERRAMIENTAS }))

server.setRequestHandler(CallToolRequestSchema, async (req) => {
  const { name, arguments: a = {} } = req.params as any
  const texto = (t: string) => ({ content: [{ type: 'text' as const, text: limpiar(t) }] })

  if (name !== 'sgs_doctor' && !existsSync(join(RAIZ, 'Cargo.toml'))) {
    return texto(
      `No encuentro el repositorio en ${RAIZ}.\n` +
        `Arranca el servidor con --repo-root /ruta/a/Stellar-Game-Studio.`
    )
  }

  const lista = (x: unknown) => (Array.isArray(x) ? x.filter(slugValido) : [])

  switch (name) {
    case 'sgs_doctor': {
      const l: string[] = [`raiz: ${RAIZ}`, `bun:  ${BUN}`, '']
      const v = await correr(['--version'], 15_000)
      l.push(v.ok ? `bun responde: ${v.salida.trim()}` : `bun NO responde: ${v.salida.split('\n')[0]}`)
      for (const f of ['Cargo.toml', 'package.json', '.env', 'deployment.json']) {
        l.push(`${existsSync(join(RAIZ, f)) ? 'esta' : 'FALTA'}  ${f}`)
      }
      l.push('', `contratos: ${contratos().join(', ') || 'ninguno'}`)
      l.push('', 'Si falta el target de Rust: rustup target add wasm32v1-none')
      return texto(l.join('\n'))
    }

    case 'sgs_list_contracts': {
      const cs = contratos()
      if (!cs.length) return texto('No hay contratos en contracts/.')
      const ids = desplegados()
      const filas = cs.map((c) => {
        const k = Object.keys(ids).find((x) =>
          x.toLowerCase().includes(c.replace(/-/g, '_').toLowerCase())
        )
        return `  ${c.padEnd(20)} ${k ? ids[k] : 'sin desplegar'}`
      })
      return texto(`Contratos:\n${filas.join('\n')}`)
    }

    case 'sgs_create': {
      if (!slugValido(a.slug)) {
        return texto(
          `"${a.slug}" no sirve. Tiene que empezar con minuscula y llevar solo ` +
            `minusculas, numeros y guiones simples.`
        )
      }
      if (RESERVADOS.has(a.slug)) {
        return texto(
          `"${a.slug}" es un contrato que ya existe en el repositorio. ` +
            `Usar ese nombre con force borraria su directorio. Elegi otro.`
        )
      }
      // --skip-setup siempre: sin el, create encadena un deploy a testnet, y una
      // herramienta que se llama "crear" no puede financiar cuentas sola.
      const args = ['run', 'create', a.slug, '--skip-setup']
      if (a.force) args.push('--force')
      const r = await correr(args, LENTO)
      return texto(r.salida || (r.ok ? 'listo' : 'fallo sin salida'))
    }

    case 'sgs_build': {
      const r = await correr(['run', 'build', ...lista(a.contracts)], LENTO)
      return texto(r.salida || (r.ok ? 'listo' : 'fallo sin salida'))
    }

    case 'sgs_test': {
      const cs = lista(a.contracts)
      const args = cs.length
        ? ['x', 'cargo', 'test', '--manifest-path', `contracts/${cs[0]}/Cargo.toml`]
        : ['x', 'cargo', 'test', '--workspace']
      const r = await correr(args, LENTO)
      return texto(r.salida || (r.ok ? 'listo' : 'fallo sin salida'))
    }

    case 'sgs_bindings': {
      const r = await correr(['run', 'bindings', ...lista(a.contracts)], MEDIO)
      return texto(r.salida || (r.ok ? 'listo' : 'fallo sin salida'))
    }

    case 'sgs_deploy': {
      if (a.confirm !== true) {
        return texto(
          'No despliego sin confirm: true.\n\n' +
            'Un deploy reescribe el .env desde plantilla, asi que se pierde ' +
            'cualquier variable que hayas agregado a mano. Confirmalo con la persona.'
        )
      }
      const antes = desplegados()
      const r = await correr(['run', 'deploy', ...lista(a.contracts)], LENTO)
      const despues = desplegados()
      const cambios = Object.entries(despues)
        .filter(([k, v]) => antes[k] !== v)
        .map(([k, v]) => `  ${k}=${v}`)
      return texto(r.salida + (cambios.length ? `\n\nIDs nuevos:\n${cambios.join('\n')}` : ''))
    }

    case 'sgs_publish': {
      if (!slugValido(a.slug)) return texto(`"${a.slug}" no sirve como nombre de juego.`)
      const args = ['run', 'publish', a.slug]
      if (a.build) args.push('--build')
      const r = await correr(args, LENTO)
      return texto(r.salida || (r.ok ? 'listo' : 'fallo sin salida'))
    }

    default:
      return texto(`No conozco "${name}".`)
  }
})

await server.connect(new StdioServerTransport())
