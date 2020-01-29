package org.mozilla.surfmanthreadsexample;

import android.opengl.GLSurfaceView.Renderer;
import javax.microedition.khronos.egl.EGLConfig;
import javax.microedition.khronos.opengles.GL10;

public class SurfmanThreadsExampleRenderer implements Renderer {
    private static native void init(SurfmanThreadsExampleResourceLoader resourceLoader,
                                    int width,
                                    int height);
    private static native void tick();
    static native void runTests();

    private final MainActivity mActivity;

    static {
        System.loadLibrary("surfman_android_threads");
    }

    SurfmanThreadsExampleRenderer(MainActivity activity) {
        mActivity = activity;
    }

    @Override
    public void onSurfaceCreated(GL10 gl10, EGLConfig eglConfig) {
    }

    @Override
    public void onSurfaceChanged(GL10 gl10, int width, int height) {
        init(new SurfmanThreadsExampleResourceLoader(mActivity.getAssets()), width, height);
    }

    @Override
    public void onDrawFrame(GL10 gl10) {
        tick();
    }
}
