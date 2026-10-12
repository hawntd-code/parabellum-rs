package mx.parabellum.juego.mc;

import net.minecraft.client.renderer.texture.NativeImage;
import com.mojang.blaze3d.systems.RenderSystem;
import mx.parabellum.juego.Imagen;
import net.minecraft.client.Minecraft;
import com.mojang.blaze3d.matrix.MatrixStack;
import net.minecraft.client.gui.AbstractGui;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.util.ResourceLocation;

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

    private static final Map<String, Entrada> CACHE = new HashMap<String, Entrada>();
    private static int contador;

    private Texturas() {
    }

    public static void dibujar(MatrixStack g, String nombre, Imagen img, int x, int y, int w, int h) {
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
        Minecraft.getInstance().getTextureManager().bind(e.id);
        RenderSystem.color4f(1, 1, 1, 1);
        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        AbstractGui.blit(g, x, y, 0, 0, w, h, w, h);
        RenderSystem.disableBlend();
    }
}
