package mx.parabellum.juego.mc;

import com.mojang.blaze3d.platform.Window;
import mx.parabellum.juego.Imagen;
import mx.parabellum.juego.Juego;
import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;

/** La pantalla del panel: dibuja la imagen del núcleo y le pasa el ratón y el teclado. */
public final class PantallaPanel extends Screen {
    private final Screen anterior;

    PantallaPanel(Screen anterior) {
        super(Component.literal("Parabellum"));
        this.anterior = anterior;
    }

    private int s() {
        return Math.max(1, (int) minecraft.getWindow().getGuiScale());
    }

    @Override
    protected void init() {
        Juego.actual().panelAbierto();
    }

    @Override
    public void render(PoseStack g, int mouseX, int mouseY, float delta) {
        renderBackground(g);
        super.render(g, mouseX, mouseY, delta);
        Juego juego = Juego.actual();
        Window w = minecraft.getWindow();
        int s = s();
        Imagen img = juego.dibujarPanel(w.getWidth(), w.getHeight(), s);
        Texturas.dibujar(g, "panel", img, juego.panelX() / s, (juego.panelY() + juego.panelDesliz(s)) / s, img.ancho / s, img.alto / s, juego.panelAlfa());
    }

    @Override
    public void mouseMoved(double x, double y) {
        Juego.actual().raton(x * s(), y * s());
    }

    @Override
    public boolean mouseClicked(double x, double y, int boton) {
        Juego.actual().clic(x * s(), y * s(), boton);
        return true;
    }

    @Override
    public boolean mouseScrolled(double x, double y, double cantidad) {
        return Juego.actual().rueda(x * s(), y * s(), cantidad);
    }

    @Override
    public boolean keyPressed(int tecla, int codigo, int mods) {
        if (Juego.actual().teclaPanel(tecla, mods)) return true;
        if (Base.ABRIR.matches(tecla, codigo)) {
            onClose();
            return true;
        }
        return super.keyPressed(tecla, codigo, mods);
    }

    @Override
    public boolean charTyped(char c, int mods) {
        return Juego.actual().caracter(c);
    }

    @Override
    public void onClose() {
        minecraft.setScreen(anterior);
    }

    @Override
    public void removed() {
        Juego.actual().panelCerrado();
        Texturas.liberar("panel");
    }

    @Override
    public boolean isPauseScreen() {
        return false;
    }
}
