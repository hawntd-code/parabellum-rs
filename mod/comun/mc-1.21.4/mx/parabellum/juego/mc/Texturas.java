package mx.parabellum.juego.mc;

import com.mojang.blaze3d.platform.NativeImage;
import mx.parabellum.juego.Imagen;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.renderer.RenderType;
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

    public static void dibujar(GuiGraphics g, String nombre, Imagen img, int x, int y, int w, int h) {
        dibujar(g, nombre, img, x, y, w, h, 1f);
    }

    /** `alfa`: de 0 (transparente) a 1; así el panel entra suave. */
    public static void dibujar(GuiGraphics g, String nombre, Imagen img, int x, int y, int w, int h, float alfa) {
        Entrada e = CACHE.get(nombre);
        Rectangle cambio;
        if (e == null || e.imagen != img) {
            if (e != null) Minecraft.getInstance().getTextureManager().release(e.id);
            e = new Entrada();
            e.imagen = img;
            e.nativa = new NativeImage(img.ancho, img.alto, false);
            e.textura = new DynamicTexture(e.nativa);
            e.id = ResourceLocation.fromNamespaceAndPath("parabellum", nombre + "_" + (contador++));
            Minecraft.getInstance().getTextureManager().register(e.id, e.textura);
            CACHE.put(nombre, e);
            img.tomarSucio();
            cambio = new Rectangle(0, 0, img.ancho, img.alto);
        } else {
            cambio = img.tomarSucio();
        }
        if (cambio != null) {
            for (int py = cambio.y; py < cambio.y + cambio.height; py++) {
                for (int px = cambio.x; px < cambio.x + cambio.width; px++) e.nativa.setPixel(px, py, img.argb(px, py));
            }
            e.textura.upload();
        }
        g.blit(RenderType::guiTextured, e.id, x, y, 0, 0, w, h, w, h, color(alfa));
    }

    /** Blanco con esa transparencia (ARGB): la textura se ve igual, solo más o menos opaca. */
    private static int color(float alfa) {
        return Math.round(Math.max(0, Math.min(1, alfa)) * 255) << 24 | 0xFFFFFF;
    }
}
