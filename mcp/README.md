# stellar-game-studio-mcp

Deja que un asistente use Stellar Game Studio: listar contratos, crear un juego,
compilar, generar bindings y desplegar a testnet.

## Instalación

```bash
claude mcp add sgs -- npx -y stellar-game-studio-mcp@latest \
  --repo-root /ruta/a/Stellar-Game-Studio \
  --bun /ruta/a/bun
```

`--bun` casi siempre hace falta. Un servidor MCP hereda un PATH mínimo y `bun`
rara vez está en él, así que sin la ruta absoluta el servidor arranca y falla en
la primera herramienta. `sgs_doctor` te dice si la ruta que pasaste responde.

El paquete se instala con npx, pero la cadena de herramientas de abajo no viene
adentro: hacen falta `bun`, `cargo`, y el target `wasm32v1-none`
(`rustup target add wasm32v1-none`).

## Herramientas

| Herramienta | Qué hace |
|---|---|
| `sgs_doctor` | Comprueba que esté todo. Empezá por acá. |
| `sgs_list_contracts` | Los contratos del workspace y sus IDs desplegados. |
| `sgs_create` | Crea un juego nuevo. No despliega. |
| `sgs_build` | Compila a `wasm32v1-none`. |
| `sgs_test` | Corre los tests de los contratos. |
| `sgs_bindings` | Genera los bindings de TypeScript. |
| `sgs_deploy` | **Despliega a testnet.** Exige `confirm: true`. |
| `sgs_publish` | Exporta el frontend de un juego. |

## Lo que el servidor no te deja hacer sin querer

**`sgs_deploy` exige `confirm: true`.** No por el costo, que en testnet es cero,
sino porque un deploy **reescribe el `.env` desde una plantilla fija**: cualquier
variable que hayas agregado a mano se pierde.

**`sgs_create` pasa siempre `--skip-setup`.** Sin ese flag, el script encadena
build, deploy y bindings, o sea que "crear un juego" terminaría desplegando a
testnet y financiando cuentas. Si querés eso, llamá a `sgs_deploy` a propósito.

**`sgs_create` rechaza los nombres de los contratos que ya existen.** El script
no los valida, y con `--force` un nombre repetido borra el directorio del
contrato original.

**Los secretos no salen.** Toda la salida pasa por un filtro que tapa las claves
de Stellar y los argumentos `--source-account` antes de devolver nada.
