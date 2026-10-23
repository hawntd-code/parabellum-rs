# Puente de CurseForge

Parabellum no lleva la clave de la API de CurseForge: el exe se reparte y cualquiera podría sacarla de
ahí. La tiene este puente, y Parabellum le pide aquí lo que necesita (buscar, ver un proyecto, sus
archivos, resolver los de un modpack). El puente se lo pregunta a CurseForge con la clave. De paso sirve
en redes que bloquean `api.curseforge.com`, como la de la escuela. Los archivos (`.jar` y `.zip`) no pasan
por aquí: se bajan directo del CDN de CurseForge (`forgecdn.net`), que esas redes sí dejan pasar.

Es una función de Google Cloud (Cloud Run functions) en el proyecto de Firebase de Parabellum
(`parabellum52`, plan Blaze), igual que el [puente de Modrinth](../puente-modrinth). Solo acepta las rutas
que usa `src-tauri/src/curseforge.rs`; si Parabellum empieza a usar otra, agrégala en `ROUTES` de
`main.py` y vuelve a publicarlo. La dirección del puente está en `BRIDGE` de `curseforge.rs`.

## Publicarlo (desde el navegador, sin instalar nada)

**Desplegado el 2026-10-22:** servicio `curseforge`, Python 3.14, punto de entrada `curseforge`,
acceso público y escalamiento de 0 a 3 instancias. Comprobadas las categorías (52) y la búsqueda real
desde su URL pública; la clave está configurada solo en la variable del servidor.

1. Abre consola con la cuenta de Firebase.
2. **Escribir una función** (*Write a function*).
3. Llénalo así:
   - Nombre del servicio: `curseforge`.
   - Región: `us-central1`.
   - Entorno de ejecución: **Python** (la versión más nueva).
   - Autenticación: **Permitir acceso público** (*Allow public access*).
   - En «Contenedores, volúmenes, redes, seguridad» › «Variables y secretos»: la variable
     `CURSEFORGE_API_KEY` con la clave de la API de CurseForge (la de la consola de CurseForge para
     terceros). **Solo aquí**: nunca en el repo ni en `config-integrada.json` del exe.
   - En «Escalado del servicio»: **máximo 3 instancias** (limita el gasto si alguien lo usa de más).
4. **Crear**. Se abre el editor:
   - en `main.py`, pega todo `main.py` de esta carpeta;
   - en `requirements.txt`, pega `requirements.txt`;
   - **Punto de entrada de la función**: `curseforge`.
5. **Guardar y volver a implementar**. Tarda 2–3 minutos.
6. La URL queda como `https://curseforge-86460886589.us-central1.run.app`. Para probarla, abre esa URL con
   `/v1/categories?gameId=432&classId=6` al final: debe salir una lista en JSON.

Si cambias la clave en CurseForge, cámbiala en la variable (Editar e implementar una revisión nueva).

## Probarlo en tu PC

```
CURSEFORGE_API_KEY=… python main.py 8792
```

Abre <http://127.0.0.1:8792/v1/categories?gameId=432&classId=6>. Con `CURSEFORGE_API=http://127.0.0.1:8793`
pregunta a otro servidor, por ejemplo un CurseForge falso para probar sin red. Para que Parabellum use ese
puente local: `PARABELLUM_CURSEFORGE_PUENTE=http://127.0.0.1:8792`.
