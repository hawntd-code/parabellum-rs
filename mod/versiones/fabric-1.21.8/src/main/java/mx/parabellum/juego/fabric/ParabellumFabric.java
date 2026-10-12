package mx.parabellum.juego.fabric;

import mx.parabellum.juego.Juego;
import mx.parabellum.juego.mc.Base;
import net.fabricmc.api.ClientModInitializer;

/** Fabric 1.21.8. La tecla, el tick y los avisos van con mixins (sin Fabric API). */
public final class ParabellumFabric extends Base implements ClientModInitializer {
    @Override
    public void onInitializeClient() {
        Juego.iniciar(this);
    }

    @Override
    protected String loader() {
        return "Fabric";
    }
}
