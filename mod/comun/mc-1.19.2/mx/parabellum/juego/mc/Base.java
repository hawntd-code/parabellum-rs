package mx.parabellum.juego.mc;

import com.mojang.blaze3d.platform.InputConstants;
import com.mojang.blaze3d.platform.Window;
import mx.parabellum.juego.Imagen;
import mx.parabellum.juego.Juego;
import mx.parabellum.juego.Plataforma;
import net.minecraft.SharedConstants;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.gui.screens.ConnectScreen;
import net.minecraft.client.gui.screens.GenericDirtMessageScreen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.client.multiplayer.resolver.ServerAddress;
import net.minecraft.network.chat.Component;
import org.lwjgl.glfw.GLFW;

import java.io.File;

/**
 * Minecraft 1.19 a 1.19.2 (lo mismo en Fabric y Forge): une el núcleo con el juego. Cada loader solo
 * registra la tecla y llama a {@link #tick} y {@link #dibujarAvisos}.
 */
public abstract class Base implements Plataforma {
    /** La tecla (en Controles › Multijugador). */
    public static final KeyMapping ABRIR = new KeyMapping("key.parabellum.abrir", InputConstants.Type.KEYSYM, GLFW.GLFW_KEY_O, "key.categories.multiplayer");

    private static boolean probado;

    /** "Fabric" o "Forge". */
    protected abstract String loader();

    protected static Minecraft mc() {
        return Minecraft.getInstance();
    }

    /** Cada tick del cliente. */
    public static void tick() {
        Juego juego = Juego.actual();
        if (juego == null) return;
        juego.tick();
        while (ABRIR.consumeClick()) juego.tecla();
        // -Dparabellum.prueba.abrir: abre el panel solo en el menú principal (para probar sin tocar nada).
        if (!probado && System.getProperty("parabellum.prueba.abrir") != null && mc().screen instanceof TitleScreen && mc().getOverlay() == null) {
            probado = true;
            juego.tecla();
        }
    }

    /** Los avisos, encima de la interfaz del juego. */
    public static void dibujarAvisos(PoseStack g) {
        Juego juego = Juego.actual();
        if (juego == null) return;
        Window w = mc().getWindow();
        int s = Math.max(1, (int) w.getGuiScale());
        Imagen img = juego.avisos(w.getWidth(), w.getHeight(), s);
        if (img == null) return;
        int margen = 8 * s;
        int x = ((w.getWidth() - img.ancho - margen) / s) * s;
        Texturas.dibujar(g, "avisos", img, x / s, margen / s, img.ancho / s, img.alto / s);
    }

    @Override
    public File carpeta() {
        return mc().gameDirectory;
    }

    @Override
    public int anchoVentana() {
        return mc().getWindow().getWidth();
    }

    @Override
    public int altoVentana() {
        return mc().getWindow().getHeight();
    }

    @Override
    public int escala() {
        return Math.max(1, (int) mc().getWindow().getGuiScale());
    }

    @Override
    public void abrirPanel() {
        mc().setScreen(new PantallaPanel(mc().screen));
    }

    @Override
    public void cerrarPanel() {
        if (mc().screen instanceof PantallaPanel panel) panel.onClose();
    }

    @Override
    public boolean panelAbierto() {
        return mc().screen instanceof PantallaPanel;
    }

    @Override
    public void conectar(String direccion) {
        Minecraft mc = mc();
        mc.execute(() -> {
            // Como el botón «Desconectar» del menú de pausa.
            if (mc.level != null) {
                boolean local = mc.isLocalServer();
                mc.level.disconnect();
                if (local) mc.clearLevel(new GenericDirtMessageScreen(Component.translatable("menu.savingLevel")));
                else mc.clearLevel();
            }
            ServerData datos = new ServerData("Parabellum", direccion, false);
            ConnectScreen.startConnecting(new TitleScreen(), mc, ServerAddress.parseString(direccion), datos);
        });
    }

    @Override
    public void enJuego(Runnable tarea) {
        mc().execute(tarea);
    }

    @Override
    public String portapapeles() {
        return mc().keyboardHandler.getClipboard();
    }

    @Override
    public void copiar(String texto) {
        mc().keyboardHandler.setClipboard(texto);
    }

    @Override
    public String descripcion() {
        return loader() + " " + SharedConstants.getCurrentVersion().getName();
    }

    @Override
    public String tecla() {
        return ABRIR.getTranslatedKeyMessage().getString();
    }
}
