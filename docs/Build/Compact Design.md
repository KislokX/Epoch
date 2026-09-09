# `/compact` — diseño de implementación

> Estado: **entrega 1 implementada** · dueña: [ADR-0012](../ADR/0012-context-composer.md) ·
> pendiente: composición especializada para proveedores de modelo.

`/compact` no es borrar el chat ni pedirle al modelo que «recuerde mejor». Es una reducción
explícita y visible del contexto de una Quest larga. El Chronicle sustituye su prefijo literal
por una memoria durable y conserva una cola reciente verificable; el siguiente turno recibe esa
representación compacta, no una segunda copia del historial.

## Promesa al usuario

- Mientras el resumen se está preparando, la conversación muestra **Compacting context** con el
  número real de registros que se están reduciendo. Al terminar, el Chronicle conserva una marca
  **Context compacted**; no es una frase atribuida al personaje ni expone el resumen privado.
- Las líneas antiguas se sustituyen por una marca visible de compactación y un resumen durable;
  no desaparecen silenciosamente ni se presentan como palabras del personaje.
- Compactar es voluntario, nunca automático.
- Epoch muestra qué tramo anterior fue sustituido y conserva las dos intervenciones más recientes
  fuera de toda compactación.
- Si no hay suficiente historial anterior, la acción explica que no hay nada que compactar.
- La respuesta de resumen no se presenta como una respuesta normal del personaje ni como evidencia
  de que el trabajo se haya realizado.

## Registro canónico

El Kernel conserva la memoria de la Quest, equivalente a:

```text
memory: {
  coversThrough: usize,
  character: CharacterId,
  summary: String,
  participants: [...],
  artifacts: [...],
  revisions: usize,
  lastContributor: CharacterId | null
}
```

`coversThrough` es la cantidad acumulada de registros que el resumen representa. Al terminar con
un resumen no vacío, Epoch reemplaza todos los registros salvo las dos intervenciones más recientes
por esa memoria. Participantes, evidencia, revisiones y el último contribuyente se conservan como
datos estructurados para que History no pierda hechos cuando ya no conserva cada frase literal.
Una compactación posterior reemplaza el resumen previo y el material nuevo por uno mejor; no se
acumulan resúmenes que se contradigan ni vuelve a crecer el archivo con el mismo pasado.

## Turno de resumen

La acción genera un turno especializado, no un mensaje del usuario:

1. El Engine selecciona únicamente el prefijo que se cubrirá; nunca las dos entradas recientes.
2. Para un agente de CLI, reanuda **la sesión existente** con una instrucción fija y sandbox de
   solo lectura: no puede usar herramientas ni modificar el proyecto.
3. Solo un resumen no vacío escribe la memoria durable; entonces, y solo entonces, Epoch olvida el handle
   externo de esa persona. Si falla, el handle anterior queda intacto y puede continuarse.
4. El siguiente mensaje abre una sesión nueva y recibe el resumen persistido más la cola literal.
   El prefijo ya no pesa como texto repetido: sobrevive como memoria compacta y como sus hechos
   estructurados.

El turno no usa capacidades, no ejecuta herramientas y no puede producir evidencia. La interfaz no
lo presenta como una respuesta del personaje: `/compact` aparece en el menú de barra y el medidor
de contexto queda sin lectura hasta que la sesión nueva reporte la suya. La misma operación para un
proveedor de modelo sigue pendiente: requiere su Conversation especializada, no una simulación de
la sesión de un agente.

Las direcciones `http` y `https` escritas en cualquier línea del Chronicle se presentan como
acciones para abrir el navegador nativo. Es una sola proyección para todos los cerebros, no una
capacidad particular de Claude, Codex u Ollama; el shell sigue rechazando cualquier otro esquema.

## Seguridad y explicabilidad

El resumen es salida de un modelo, no verdad nueva. Por tanto conserva la procedencia de
conversación y lleva el personaje que lo produjo. El texto de herramientas incluido en el tramo
original sigue siendo no autoritativo durante la construcción del turno de resumen; no se lo
asciende a instrucciones.

La proyección de contexto conservará su `Report`: incluirá un bloque `compact-N`, qué entradas
cubre y el motivo de que el resto quede fuera. La interfaz podrá decir «compactadas entradas 1–24;
las 25–26 permanecen literales» sin calcularlo por su cuenta.

## Cobertura actual y siguiente corte

1. **Hecho:** El prefijo se sustituye solo después de un resumen no vacío; un fallo deja las líneas
   literales y la sesión anterior intactas.
2. **Hecho:** La composición usa una sola memoria durable y conserva siempre los dos registros más
   recientes.
3. **Hecho:** Un segundo resumen sustituye la memoria previa y el material nuevo, sin volver a
   persistir las conversaciones ya reducidas.
4. **Hecho para agentes:** el resumen sale vacío o falla, y por tanto no se escribe ni se rota la
   sesión; al completarse se retira la lectura del medidor anterior.
5. **Siguiente:** el turno especializado de proveedor y la misma garantía de fallo sin mutación.
