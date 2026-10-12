package mx.parabellum.juego.mc;

import com.mojang.blaze3d.platform.NativeImage;
import com.mojang.blaze3d.systems.RenderSystem;
import mx.parabellum.juego.Imagen;
import net.minecraft.client.Minecraft;
import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.gui.GuiComponent;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.resources.ResourceLocation;

import java.awt.Rectangle;
import java.util.HashMap;
import java.util.Map;

/** Sube las imágenes del núcleo como texturas (solo la parte que cambió) y las dibuja. */
public final class Texturas {
    private static final class Entrada {
        Imagen imagen;
        NativeImage nativa;
        DynamicTexture textura;
        ResourceLocation id;
    }

    private static final Map<String, Entrada> CACHE = new HashMap<>();
    private static int contador;

    private Texturas() {
    }

    /** Al cerrar el panel: su textura se suelta (la imagen se vuelve a hacer al abrirlo). */
    public static void liberar(String nombre) {
        Entrada e = CACHE.remove(nombre);
        if (e != null) Minecraft.getInstance().getTextureManager().release(e.id);
    }

    public static void dibujar(PoseStack g, String nombre, Imagen img, int x, int y, int w, int h) {
        dibujar(g, nombre, img, x, y, w, h, 1f);
    }

    /** `alfa`: de 0 (transparente) a 1; así el panel entra suave. */
    public static void dibujar(PoseStack g, String nombre, Imagen img, int x, int y, int w, int h, float alfa) {
        Entrada e = CACHE.get(nombre);
        Rectangle cambio;
        if (e == null || e.imagen != img) {
            if (e != null) Minecraft.getInstance().getTextureManager().release(e.id);
            e = new Entrada();
            e.imagen = img;
            e.nativa = new NativeImage(img.ancho, img.alto, false);
            e.textura = new DynamicTexture(e.nativa);
            e.id = new ResourceLocation("parabellum", nombre + "_" + (contador++));
            Minecraft.getInstance().getTextureManager().register(e.id, e.textura);
            CACHE.put(nombre, e);
            img.tomarSucio();
            cambio = new Rectangle(0, 0, img.ancho, img.alto);
        } else {
            cambio = img.tomarSucio();
        }
        if (cambio != null) {
            for (int py = cambio.y; py < cambio.y + cambio.height; py++) {
                for (int px = cambio.x; px < cambio.x + cambio.width; px++) e.nativa.setPixelRGBA(px, py, img.abgr(px, py));
            }
            e.textura.upload();
        }
        RenderSystem.setShader(GameRenderer::getPositionTexShader);
        RenderSystem.setShaderTexture(0, e.id);
        RenderSystem.setShaderColor(1, 1, 1, alfa);
        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        GuiComponent.blit(g, x, y, 0, 0, w, h, w, h);
        RenderSystem.disableBlend();
        RenderSystem.setShaderColor(1, 1, 1, 1);
    }
}
