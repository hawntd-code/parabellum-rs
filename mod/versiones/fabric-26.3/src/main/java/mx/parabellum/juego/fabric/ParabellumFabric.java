package mx.parabellum.juego.fabric;

import com.mojang.blaze3d.platform.InputConstants;
import com.mojang.blaze3d.platform.Window;
import mx.parabellum.juego.Imagen;
import mx.parabellum.juego.Juego;
import mx.parabellum.juego.Plataforma;
import net.fabricmc.api.ClientModInitializer;
import net.minecraft.SharedConstants;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.ConnectScreen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.client.multiplayer.resolver.ServerAddress;
import net.minecraft.network.chat.Component;

import java.io.File;

/** Fabric 26.3: une el núcleo con el juego. */
public final class ParabellumFabric implements ClientModInitializer, Plataforma {
    /** La tecla (en Controles › Multijugador); la registra OptionsMixin. */
    public static final KeyMapping ABRIR = new KeyMapping("key.parabellum.abrir", InputConstants.Type.KEYBOARD, InputConstants.KEY_O, KeyMapping.Category.MULTIPLAYER);

    @Override
    public void onInitializeClient() {
        Juego.iniciar(this);
    }

    private static Minecraft mc() {
        return Minecraft.getInstance();
    }

    private static boolean probado;

    /** Cada tick del cliente (MinecraftMixin). */
    public static void tick() {
        Juego juego = Juego.actual();
        if (juego == null) return;
        juego.tick();
        while (ABRIR.consumeClick()) juego.tecla();
        // -Dparabellum.prueba.abrir: abre el panel solo en el menú principal (para probar sin tocar nada).
        if (!probado && System.getProperty("parabellum.prueba.abrir") != null && mc().gui.screen() instanceof TitleScreen && mc().gui.overlay() == null) {
            probado = true;
            juego.tecla();
        }
    }

    /** Los avisos, encima de la interfaz del juego (HudMixin). */
    public static void dibujarAvisos(GuiGraphicsExtractor g) {
        Juego juego = Juego.actual();
        if (juego == null) return;
        Window w = mc().getWindow();
        int s = Math.max(1, w.getGuiScale());
        Imagen img = juego.avisos(w.getWidth(), w.getHeight(), s);
        if (img == null) return;
        int margen = 8 * s;
        int x = ((w.getWidth() - img.ancho - margen) / s) * s;
        Texturas.dibujar(g, "avisos", img, x / s, margen / s, img.ancho / s, img.alto / s);
    }

    // ================= Plataforma

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
        return Math.max(1, mc().getWindow().getGuiScale());
    }

    @Override
    public void abrirPanel() {
        mc().gui.setScreen(new PantallaPanel(mc().gui.screen()));
    }

    @Override
    public void cerrarPanel() {
        if (mc().gui.screen() instanceof PantallaPanel panel) panel.onClose();
    }

    @Override
    public boolean panelAbierto() {
        return mc().gui.screen() instanceof PantallaPanel;
    }

    @Override
    public void conectar(String direccion) {
        Minecraft mc = mc();
        mc.execute(() -> {
            // Como el botón «Desconectar» del menú de pausa (guarda el mundo propio si lo hay).
            if (mc.level != null) mc.disconnectFromWorld(Component.translatable("menu.savingLevel"));
            ServerData datos = new ServerData("Parabellum", direccion, ServerData.Type.OTHER);
            ConnectScreen.startConnecting(new TitleScreen(), mc, ServerAddress.parseString(direccion), datos, false, null);
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
        return "Fabric " + SharedConstants.getCurrentVersion().name();
    }

    @Override
    public String tecla() {
        return ABRIR.getTranslatedKeyMessage().getString();
    }
}
