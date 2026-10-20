# Puente de Modrinth

En redes que bloquean la API de Modrinth (`api.modrinth.com`), como la de la escuela, Parabellum le pide
la información de los mods a este puente, y el puente se la pregunta a Modrinth. Los archivos (`.jar` y
`.mrpack`) no pasan por aquí: se bajan directo de `cdn.modrinth.com`, que esas redes sí dejan pasar.

Es una función de Google Cloud (Cloud Run functions) en el proyecto de Firebase de Parabellum
(`parabellum52`, plan Blaze). Solo acepta las rutas que usa `src-tauri/src/modrinth.rs`; si Parabellum
empieza a usar otra, agrégala en `ROUTES` de `main.py` y vuelve a publicarlo.

Parabellum primero va directo a Modrinth. Si la red lo bloquea, usa el puente sin preguntar (y en modo
escuela, el puente desde el principio). La dirección del puente está en `PUENTE` de `modrinth.rs`.

## Publicarlo (desde el navegador, sin instalar nada)

1. Abre <https://console.cloud.google.com/run?project=parabellum52> con la cuenta de Firebase.
2. **Escribir una función** (*Write a function*). Si pide habilitar APIs (Cloud Run, Cloud Build,
   Artifact Registry), acepta.
3. Llénalo así:
   - Nombre del servicio: `modrinth`.
   - Región: `us-central1`.
   - Entorno de ejecución: **Python** (la versión más nueva).
   - Autenticación: **Permitir acceso público** (*Allow public access*).
   - En «Contenedores, volúmenes, redes, seguridad» › «Escalado del servicio»: **máximo 3 instancias**
     (limita el gasto si alguien lo usa de más).
4. **Crear**. Se abre el editor:
   - en `main.py`, pega todo `main.py` de esta carpeta;
   - en `requirements.txt`, pega `requirements.txt`;
   - **Punto de entrada de la función**: `modrinth`.
5. **Guardar y volver a implementar**. Tarda 2–3 minutos.
6. Copia la URL que aparece arriba (`https://modrinth-….run.app`). Para probarla, abre esa URL con
   `/v2/tag/category` al final: debe salir una lista en JSON.

Cada búsqueda es una llamada a la función. La capa gratis de Cloud Run es mucho más de lo que usa un
grupo de la escuela.

## Probarlo en tu PC

```
python main.py 8790
```

Abre <http://127.0.0.1:8790/v2/tag/category>. Con `MODRINTH_API=http://127.0.0.1:8791` pregunta a otro
servidor, por ejemplo un Modrinth falso para probar sin red. Para que Parabellum use ese puente local:
`PARABELLUM_MODRINTH_PUENTE=http://127.0.0.1:8790`.
