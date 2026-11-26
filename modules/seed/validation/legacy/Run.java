// Offline test harness for a locally supplied vanilla server. Not shipped with the app.
package helper;
import java.net.*;
import java.nio.file.*;
public class Run {
  public static void main(String[] args) throws Exception {
    var child =
        new URLClassLoader(new URL[] {Path.of(".").toUri().toURL()}, Run.class.getClassLoader());
    Class.forName("Oracle", true, child)
        .getMethod("main", String[].class)
        .invoke(null, (Object) args);
  }
}
