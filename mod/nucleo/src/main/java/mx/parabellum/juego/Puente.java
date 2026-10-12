package mx.parabellum.juego;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Conexión con Parabellum (src-tauri/src/puente.rs): una línea JSON por mensaje por 127.0.0.1. Se
 * reconecta sola si Parabellum se cierra y se vuelve a abrir. Las respuestas y los eventos se atienden
 * en el hilo del juego (ver {@link #atender()}).
 */
final class Puente {
    interface Respuesta {
        /** datos es null si hubo error. */
        void listo(JsonElement datos, String error);
    }

    interface Eventos {
        void evento(String tipo, JsonObject datos);

        void conexion(boolean conectado);
    }

    private final int puerto;
    private final String clave;
    private final Eventos eventos;
    private final ConcurrentLinkedQueue<Runnable> pendientes = new ConcurrentLinkedQueue<Runnable>();
    private final Map<Integer, Respuesta> esperando = new ConcurrentHashMap<Integer, Respuesta>();
    private final AtomicInteger siguiente = new AtomicInteger(1);
    private volatile Socket socket;
    private volatile OutputStream salida;
    private volatile boolean conectado;

    Puente(int puerto, String clave, Eventos eventos) {
        this.puerto = puerto;
        this.clave = clave;
        this.eventos = eventos;
        Thread hilo = new Thread(new Runnable() {
            public void run() {
                bucle();
            }
        }, "Parabellum (puente)");
        hilo.setDaemon(true);
        hilo.start();
    }

    /** Lee -Dparabellum.puente y -Dparabellum.clave (los pone Parabellum al abrir el juego). */
    static Puente desdePropiedades(Eventos eventos) {
        String puerto = System.getProperty("parabellum.puente");
        String clave = System.getProperty("parabellum.clave");
        if (puerto == null || clave == null) return null;
        try {
            return new Puente(Integer.parseInt(puerto.trim()), clave.trim(), eventos);
        } catch (NumberFormatException e) {
            return null;
        }
    }

    boolean conectado() {
        return conectado;
    }

    /** Pide algo a Parabellum; la respuesta llega en el hilo del juego. */
    void pedir(String op, JsonObject datos, Respuesta respuesta) {
        JsonObject mensaje = datos == null ? new JsonObject() : datos;
        int id = siguiente.getAndIncrement();
        mensaje.addProperty("op", op);
        mensaje.addProperty("id", id);
        if (respuesta != null) esperando.put(id, respuesta);
        if (!enviar(mensaje) && respuesta != null) {
            esperando.remove(id);
            final Respuesta r = respuesta;
            pendientes.add(new Runnable() {
                public void run() {
                    r.listo(null, T.t("Parabellum no está abierto.", "Parabellum is not open."));
                }
            });
        }
    }

    void pedir(String op, JsonObject datos) {
        pedir(op, datos, null);
    }

    /** En el hilo del juego: lo que llegó de Parabellum. */
    void atender() {
        Runnable tarea;
        while ((tarea = pendientes.poll()) != null) {
            try {
                tarea.run();
            } catch (RuntimeException e) {
                e.printStackTrace();
            }
        }
    }

    private synchronized boolean enviar(JsonObject mensaje) {
        OutputStream out = salida;
        if (out == null) return false;
        try {
            out.write((mensaje.toString() + "\n").getBytes(StandardCharsets.UTF_8));
            out.flush();
            return true;
        } catch (Exception e) {
            cerrar();
            return false;
        }
    }

    private void cerrar() {
        Socket s = socket;
        socket = null;
        salida = null;
        try {
            if (s != null) s.close();
        } catch (Exception ignored) {
        }
    }

    private void bucle() {
        while (true) {
            try {
                Socket s = new Socket();
                s.connect(new InetSocketAddress("127.0.0.1", puerto), 3000);
                s.setTcpNoDelay(true);
                socket = s;
                salida = s.getOutputStream();
                JsonObject hola = new JsonObject();
                hola.addProperty("clave", clave);
                pedir("hola", hola, new Respuesta() {
                    public void listo(JsonElement datos, String error) {
                        conectado = error == null;
                        eventos.conexion(conectado);
                    }
                });
                BufferedReader lector = new BufferedReader(new InputStreamReader(s.getInputStream(), StandardCharsets.UTF_8));
                String linea;
                while ((linea = lector.readLine()) != null) recibir(linea);
            } catch (Exception ignored) {
                // Parabellum cerrado o reiniciándose: se reintenta.
            }
            cerrar();
            if (conectado) {
                conectado = false;
                pendientes.add(new Runnable() {
                    public void run() {
                        eventos.conexion(false);
                    }
                });
            }
            for (final Map.Entry<Integer, Respuesta> e : esperando.entrySet()) {
                pendientes.add(new Runnable() {
                    public void run() {
                        e.getValue().listo(null, T.t("Se perdió la conexión con Parabellum.", "The connection with Parabellum was lost."));
                    }
                });
            }
            esperando.clear();
            try {
                Thread.sleep(2000);
            } catch (InterruptedException e) {
                return;
            }
        }
    }

    @SuppressWarnings("deprecation")
    private void recibir(String linea) {
        final JsonObject mensaje;
        try {
            mensaje = new JsonParser().parse(linea).getAsJsonObject();
        } catch (Exception e) {
            return;
        }
        if (mensaje.has("re") && !mensaje.get("re").isJsonNull()) {
            final Respuesta r = esperando.remove(mensaje.get("re").getAsInt());
            if (r == null) return;
            final String error = mensaje.has("error") ? mensaje.get("error").getAsString() : null;
            final JsonElement datos = mensaje.has("datos") ? mensaje.get("datos") : null;
            pendientes.add(new Runnable() {
                public void run() {
                    r.listo(error == null ? datos : null, error);
                }
            });
        } else if (mensaje.has("ev")) {
            final String tipo = mensaje.get("ev").getAsString();
            pendientes.add(new Runnable() {
                public void run() {
                    eventos.evento(tipo, mensaje);
                }
            });
        }
    }
}
