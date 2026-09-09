# Phase 4.3 — Compactación física del Chronicle

**Date:** 2026-08-09  
**Outcome:** `/compact` deja de ser sólo una proyección de contexto. Tras un resumen válido,
reduce el documento de la Quest de manera durable y conservadora.

## Contrato

Una Quest compactada contiene:

- `memory.summary`: el breve factual que representa el prefijo anterior;
- `memory.coversThrough`: cuántos registros representa en total;
- participantes, evidencia, revisiones y último contribuyente preservados como datos;
- `chronicle`: únicamente la cola literal reciente (dos registros al momento de compactar y lo
  que ocurra después).

Esto no trata el resumen como una respuesta del NPC ni como una segunda fuente de verdad. Es la
representación durable del pasado que el usuario eligió reducir. La UI muestra una marca de
compactación y conserva el tramo reciente; los modelos, agentes y handoffs reciben la memoria más
esa cola.

## Seguridad

- No se toca el Chronicle hasta tener un resumen no vacío.
- Un fallo de proveedor/agente conserva las líneas y la sesión anterior.
- Una nueva reducción exige que el Composer haya incluido tanto `quest-memory` como cada registro
  que pretende cubrir; de lo contrario se rechaza, nunca resume una parte y la presenta como todo.
- La evidencia, los participantes, las revisiones y el último contribuyente se trasladan al
  roll-up estructurado, para que History no se vuelva ficción cuando las frases dejan de ser
  literales.

## Evidencia inicial

Las pruebas del Kernel verifican que la reducción elimina el prefijo, mantiene el tail y preserva
los hechos. Las pruebas del Composer verifican que el siguiente turno ve el resumen y el tail,
pero no las líneas ya sustituidas. Las pruebas existentes de la marca `Compacted` siguen pasando
para abrir Quests de entrega 1 que todavía usen el formato anterior.

La confirmación pendiente es visual: ejecutar `/compact` en una Quest real, reiniciar Epoch y
comprobar que el archivo de esa Quest disminuye, la marca permanece visible y el siguiente modelo
continúa con el objetivo correcto.
